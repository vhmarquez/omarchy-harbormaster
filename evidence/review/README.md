# M0 verification history

This directory retains failed reviews and their resolutions, rather than relabeling an early green suite as sufficient. These are artifact/code reviews, not owner design/license approval, M0 completion, or production certification.

- `core-before-fix.json`: initial privacy/schema-version findings. Fixed in a fresh TDD context; 10 parent checker tests passed. The subsequent full stable review found no remaining core/harness blocker.
- `stable-before-ui-fix.json`: full stable-path review; the sole blocker is unapplied settings lost during masking. `design-draft-fix-result.json` records the fresh-context fix and seven RED→GREEN UI regressions.
- `final-design-review.json`: independent PASS clearing that exact remaining stable-path blocker; all 15 Node tests passed, and staged old HTML still reproduces all seven failures.
- `runtime-fix.json`: cleanup timeout short-circuit fix and six fault tests. The original runtime review log was truncated; this result and actual regression tests preserve the finding without manufacturing a complete original verdict.
- `final-runtime-review.json`: independent PASS for fixed runtime, current source hashes and narrow evidence claims. Aggregate cleanup deadline remains a non-blocking future hardening item.

Parent independently reran **41 tests** plus archive/fixture integrity checks; see `../parent/final-tests.json` and `../../docs/M0-PARENT-VERIFICATION.md`. The latest actual runtime runs are canonical spike evidence and identical `../parent/post-review-runtime-*.json` copies: copies are not extra independent runs. Earlier parent runtime files and `pre-review-tests.json` are historical. `../parent/final-cleanup.json` independently reads back the latest exact unit names as unloaded.

Core/harness stable review + final design re-review + final runtime review together cover the initial artifact publication. No blocking finding remains in those scopes. The intentionally unchanged archived OFL whitespace warning is not authored whitespace. No installed Rust/QML application, performance achievement, Codex trusted callback, or personal user approval is implied.
