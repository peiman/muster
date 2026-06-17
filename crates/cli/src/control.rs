//! `control` command handlers.

use crate::root::ControlSub;
use crate::store;
use crate::view::{Listing, WithNext};
use domain::Evidence;
use infrastructure::output::Output;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

pub fn execute(sub: ControlSub, output: &Output) -> Boxed {
    let dir = store::data_dir();
    match sub {
        ControlSub::Add {
            id,
            title,
            clause_ref,
            applicable,
        } => {
            let mut s = store::load(&dir)?;
            s.add_control(&id, &title, clause_ref, applicable.unwrap_or(true))?;
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            render(
                output,
                "control add",
                c,
                format!(
                    "muster control set-status {id} implemented  |  muster process link-control <pid> {id}"
                ),
            )
        }
        ControlSub::List => {
            let s = store::load(&dir)?;
            let items = s.list_controls();
            let view = Listing::new(items, "control", "muster control show <id>".to_string());
            output.success("control list", &view, &mut io::stdout())?;
            Ok(())
        }
        ControlSub::Show { id } => {
            let s = store::load(&dir)?;
            let c = s.control(&id)?;
            render(output, "control show", c, "muster readiness".to_string())
        }
        ControlSub::SetStatus { id, status } => {
            let mut s = store::load(&dir)?;
            s.set_control_status(&id, status)?;
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            render(
                output,
                "control set-status",
                c,
                format!("muster control attach-evidence {id} note \"<proof>\""),
            )
        }
        ControlSub::AttachEvidence { id, kind, value } => {
            let mut s = store::load(&dir)?;
            s.attach_control_evidence(&id, Evidence { kind, value })?;
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            render(
                output,
                "control attach-evidence",
                c,
                "muster readiness".to_string(),
            )
        }
    }
}

fn render(output: &Output, command: &str, control: &domain::Control, next: String) -> Boxed {
    let view = WithNext::new(control, next);
    output.success(command, &view, &mut io::stdout())?;
    Ok(())
}
