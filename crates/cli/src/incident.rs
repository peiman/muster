//! `incident` command handlers — command & control.

use crate::root::IncidentSub;
use crate::store;
use crate::view::{Listing, WithNext};
use infrastructure::output::Output;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

pub fn execute(sub: IncidentSub, output: &Output) -> Boxed {
    let dir = store::data_dir();
    match sub {
        IncidentSub::Report {
            id,
            title,
            severity,
            process,
        } => {
            let mut s = store::load(&dir)?;
            s.report_incident(&id, &title, severity.unwrap_or_default(), process)?;
            store::save(&dir, &s)?;
            let i = s.incident(&id)?;
            render(
                output,
                "incident report",
                i,
                format!("muster incident log {id} \"<note>\""),
            )
        }
        IncidentSub::List => {
            let s = store::load(&dir)?;
            let items = s.list_incidents();
            let view = Listing::new(items, "incident", "muster incident show <id>".to_string());
            output.success("incident list", &view, &mut io::stdout())?;
            Ok(())
        }
        IncidentSub::Show { id } => {
            let s = store::load(&dir)?;
            let i = s.incident(&id)?;
            render(
                output,
                "incident show",
                i,
                "muster incident log <id> \"<note>\"".to_string(),
            )
        }
        IncidentSub::Log { id, note } => {
            let mut s = store::load(&dir)?;
            let ts = store::now_iso();
            s.log_incident(&id, &note, &ts)?;
            store::save(&dir, &s)?;
            let i = s.incident(&id)?;
            render(
                output,
                "incident log",
                i,
                format!(
                    "muster incident close {id}  |  muster nonconformity raise <id> --from-incident {id} --description <d>"
                ),
            )
        }
        IncidentSub::Close { id } => {
            let mut s = store::load(&dir)?;
            s.close_incident(&id)?;
            store::save(&dir, &s)?;
            let i = s.incident(&id)?;
            render(output, "incident close", i, "muster readiness".to_string())
        }
    }
}

fn render(output: &Output, command: &str, incident: &domain::Incident, next: String) -> Boxed {
    let view = WithNext::new(incident, next);
    output.success(command, &view, &mut io::stdout())?;
    Ok(())
}
