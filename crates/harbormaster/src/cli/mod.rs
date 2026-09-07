//! Operational CLI boundary; argument interpretation happens before state access.
mod run;
use crate::{CliError, projects::CatalogRequest};
pub(crate) use run::state_home;
pub use run::{OperationError, execute};
mod runtime;
pub use runtime::execute_runtime;
use std::ffi::OsString;

/// Parse only the bounded registry command grammar, without I/O.
/// # Errors
/// Invalid command shape, encoding, identifiers or bounded metadata.
pub fn parse(args: &[OsString]) -> Result<CatalogRequest, CliError> {
    if args.len() > 8 || args.iter().any(|arg| arg.len() > 4096) {
        return Err(CliError::InvalidCommand);
    }
    let args = args
        .iter()
        .map(|arg| arg.to_str().ok_or(CliError::InvalidUnicode))
        .collect::<Result<Vec<_>, _>>()?;
    let request = match args.as_slice() {
        ["project", "add", root, label] => CatalogRequest::AddProject {
            root: value(root)?,
            label: value(label)?,
        },
        ["project", "list"] => CatalogRequest::Projects { after: None },
        ["project", "list", after] => CatalogRequest::Projects {
            after: Some(value(after)?),
        },
        ["preset", "add", name, executable, profile] => CatalogRequest::AddPreset {
            name: (*name).to_owned(),
            executable: value(executable)?,
            profile: (*profile).to_owned(),
        },
        ["preset", "list"] => CatalogRequest::Presets { after: None },
        ["preset", "list", after] => CatalogRequest::Presets {
            after: Some((*after).to_owned()),
        },
        ["task", "add", project, preset, label] => CatalogRequest::AddTask {
            project_id: value(project)?,
            preset: (*preset).to_owned(),
            label: value(label)?,
        },
        ["task", "list", project] => CatalogRequest::Tasks {
            project_id: value(project)?,
            after: None,
        },
        ["task", "list", project, after] => CatalogRequest::Tasks {
            project_id: value(project)?,
            after: Some(value(after)?),
        },
        ["task", "plan", id] => CatalogRequest::Task { id: value(id)? },
        _ => return Err(CliError::InvalidCommand),
    };
    request.validate().map_err(|_| CliError::InvalidCommand)?;
    Ok(request)
}

fn value<T: std::str::FromStr>(text: &str) -> Result<T, CliError> {
    text.parse().map_err(|_| CliError::InvalidCommand)
}
