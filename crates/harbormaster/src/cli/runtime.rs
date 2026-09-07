use crate::manager::{self, Command, ManagerError, RunAction};
use std::ffi::OsString;

/// Execute the bounded manager/runtime CLI grammar.
/// # Errors
/// Invalid arguments are rejected before any manager connection or state I/O.
pub fn execute_runtime(args: &[OsString]) -> Result<Vec<u8>, ManagerError> {
    if args.len() > 8 || args.iter().any(|arg| arg.len() > 4096) {
        return Err(ManagerError::InvalidRequest);
    }
    let args = args
        .iter()
        .map(|arg| arg.to_str().ok_or(ManagerError::InvalidRequest))
        .collect::<Result<Vec<_>, _>>()?;
    let command = match args.as_slice() {
        ["manager", "serve"] => {
            manager::serve()?;
            return Ok(Vec::new());
        }
        ["manager", "status"] => Command::Status,
        ["manager", "stop"] => Command::Stop,
        ["run", rest @ ..] => run_command(rest)?,
        ["runtime-worker", directory] => {
            crate::runtime::worker(std::path::Path::new(directory))?;
            return Ok(Vec::new());
        }
        _ => return Err(ManagerError::InvalidRequest),
    };
    manager::client::execute(command)
}

fn action(id: &str, action: RunAction) -> Result<Command, ManagerError> {
    Ok(Command::RunAction {
        run_id: id.parse().map_err(|_| ManagerError::InvalidRequest)?,
        action,
    })
}

fn run_command(args: &[&str]) -> Result<Command, ManagerError> {
    Ok(match args {
        ["launch", task, "--logout-policy", "existing"] => Command::Launch {
            task_id: task.parse().map_err(|_| ManagerError::InvalidRequest)?,
            logout_policy: manager::LogoutPolicy::ExistingUserManager,
            allow_shared_checkout: false,
        },
        [
            "launch",
            task,
            "--logout-policy",
            "existing",
            "--allow-shared-checkout",
        ] => Command::Launch {
            task_id: task.parse().map_err(|_| ManagerError::InvalidRequest)?,
            logout_policy: manager::LogoutPolicy::ExistingUserManager,
            allow_shared_checkout: true,
        },
        ["end", id, "--confirm"] => action(id, RunAction::End)?,
        [verb @ ("actions" | "open" | "attach" | "resume"), id] => action(
            id,
            match *verb {
                "actions" => RunAction::Actions,
                "open" => RunAction::Open,
                "attach" => RunAction::Attach,
                _ => RunAction::Resume,
            },
        )?,
        ["list", project] => Command::Runs {
            project_id: project.parse().map_err(|_| ManagerError::InvalidRequest)?,
            after: None,
        },
        ["list", project, after] => Command::Runs {
            project_id: project.parse().map_err(|_| ManagerError::InvalidRequest)?,
            after: Some(after.parse().map_err(|_| ManagerError::InvalidRequest)?),
        },
        _ => return Err(ManagerError::InvalidRequest),
    })
}
