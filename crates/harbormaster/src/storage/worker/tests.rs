//! Deterministic private dispatcher fixtures; real SQLite lives in integration tests.
use super::*;
use std::sync::mpsc;

fn request() -> Request {
    Request::Producer("11111111-1111-4111-8111-111111111111".parse().unwrap())
}

fn submitted(pool: &Pool, sender: &SyncSender<Envelope>) -> Ticket {
    pool.submit(sender, request())
        .unwrap_or_else(|error| panic!("{:?}", error.reason))
}

#[test]
fn all_outstanding_tickets_and_queued_work_share_one_hard_limit() {
    let pool = Pool::new();
    let (sender, receiver) = mpsc::sync_channel(MAX_OUTSTANDING);
    let mut tickets: Vec<_> = (0..MAX_OUTSTANDING)
        .map(|_| submitted(&pool, &sender))
        .collect();
    assert_eq!(pool.outstanding(), MAX_OUTSTANDING);
    assert_eq!(
        pool.submit(&sender, request()).err().unwrap().reason,
        SubmitFailure::Full
    );
    drop(tickets.pop());
    assert_eq!(
        pool.outstanding(),
        MAX_OUTSTANDING,
        "queued work still holds the permit"
    );
    let envelope = receiver.recv().unwrap();
    envelope.reply.send(Ok(Response::Producer(None))).unwrap();
    drop(envelope);
    assert_eq!(
        pool.outstanding(),
        MAX_OUTSTANDING,
        "completed retained ticket holds the permit"
    );
    assert_eq!(
        tickets[0].wait_timeout(Duration::ZERO),
        Ok(Response::Producer(None))
    );
    assert_eq!(
        pool.outstanding(),
        MAX_OUTSTANDING,
        "received but retained ticket stays bounded"
    );
    drop(tickets.remove(0));
    assert_eq!(pool.outstanding(), MAX_OUTSTANDING - 1);
    let _replacement = submitted(&pool, &sender);
    assert_eq!(pool.outstanding(), MAX_OUTSTANDING);
}

#[test]
fn timeout_keeps_the_same_request_and_can_later_observe_its_result() {
    let pool = Pool::new();
    let (sender, receiver) = mpsc::sync_channel(MAX_OUTSTANDING);
    let mut ticket = submitted(&pool, &sender);
    assert_eq!(
        ticket.wait_timeout(Duration::ZERO),
        Err(ReceiptError::Pending)
    );
    assert_eq!(ticket.request(), &request());
    let envelope = receiver.recv().unwrap();
    envelope.reply.send(Ok(Response::Producer(None))).unwrap();
    drop(envelope);
    assert_eq!(
        ticket.wait_timeout(Duration::from_secs(1)),
        Ok(Response::Producer(None))
    );
    assert_eq!(
        ticket.wait_timeout(Duration::ZERO),
        Err(ReceiptError::AlreadyReceived)
    );
    assert_eq!(pool.outstanding(), 1);
    drop(ticket);
    assert_eq!(pool.outstanding(), 0);
}

#[test]
fn disconnected_execution_is_unknown_and_never_a_rollback_claim() {
    let pool = Pool::new();
    let (sender, receiver) = mpsc::sync_channel(MAX_OUTSTANDING);
    let mut ticket = submitted(&pool, &sender);
    drop(receiver);
    assert_eq!(
        ticket.wait_timeout(Duration::ZERO),
        Err(ReceiptError::UnknownOutcome)
    );
    assert_eq!(ticket.request(), &request());
    assert_eq!(pool.outstanding(), 1);
}

#[test]
fn full_or_closed_queue_returns_an_unqueued_request_without_leaking_a_permit() {
    let pool = Pool::new();
    let (sender, receiver) = mpsc::sync_channel(1);
    let _ticket = submitted(&pool, &sender);
    let rejected = pool.submit(&sender, request()).err().unwrap();
    assert_eq!(rejected.reason, SubmitFailure::Full);
    assert_eq!(rejected.request, request());
    assert_eq!(pool.outstanding(), 1);
    drop(receiver);
    let rejected = pool.submit(&sender, request()).err().unwrap();
    assert_eq!(rejected.reason, SubmitFailure::Stopped);
    assert_eq!(rejected.request, request());
    assert_eq!(pool.outstanding(), 1);
}

#[test]
fn a_reply_has_only_one_slot_and_storage_errors_are_reported_once() {
    let pool = Pool::new();
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut ticket = submitted(&pool, &sender);
    let envelope = receiver.recv().unwrap();
    envelope
        .reply
        .try_send(Err(StorageError::StaleRevision))
        .unwrap();
    assert!(matches!(
        envelope.reply.try_send(Ok(Response::Producer(None))),
        Err(mpsc::TrySendError::Full(_))
    ));
    assert_eq!(
        ticket.wait_timeout(Duration::ZERO),
        Err(ReceiptError::Storage(StorageError::StaleRevision))
    );
    assert_eq!(
        ticket.wait_timeout(Duration::ZERO),
        Err(ReceiptError::AlreadyReceived)
    );
}

#[test]
fn dispatcher_is_serial_and_dropping_a_ticket_does_not_cancel_queued_work() {
    let pool = Pool::new();
    let (sender, receiver) = mpsc::sync_channel(MAX_OUTSTANDING);
    let stop = Arc::new(AtomicBool::new(false));
    let (entered, started) = mpsc::sync_channel(1);
    let (release, proceed) = mpsc::sync_channel(1);
    let first = submitted(&pool, &sender);
    let mut second = submitted(&pool, &sender);
    drop(first);
    let stopped = Arc::clone(&stop);
    let worker = thread::spawn(move || {
        let mut calls = 0;
        serve(&receiver, &stopped, |_| {
            calls += 1;
            if calls == 1 {
                entered.send(()).unwrap();
                proceed.recv_timeout(Duration::from_secs(2)).unwrap();
            }
            Ok(Response::Producer(None))
        });
        calls
    });
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(pool.outstanding(), 2);
    assert_eq!(
        second.wait_timeout(Duration::ZERO),
        Err(ReceiptError::Pending)
    );
    release.send(()).unwrap();
    assert_eq!(
        second.wait_timeout(Duration::from_secs(2)),
        Ok(Response::Producer(None))
    );
    stop.store(true, Ordering::Release);
    drop(sender);
    assert_eq!(worker.join().unwrap(), 2);
}
