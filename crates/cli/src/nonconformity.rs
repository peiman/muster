//! `nonconformity` command handlers — the refuting-signal ledger.

use crate::root::NonconformitySub;
use crate::store;
use crate::view::{Listing, WithNext};
use infrastructure::output::Output;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

pub fn execute(sub: NonconformitySub, output: &Output) -> Boxed {
    let dir = store::data_dir();
    match sub {
        NonconformitySub::Raise {
            id,
            description,
            from_incident,
            process,
            control,
            source,
        } => {
            let mut s = store::load(&dir)?;
            s.raise_nonconformity(
                &id,
                &description,
                source.unwrap_or_default(),
                from_incident,
                process,
                control,
            )?;
            store::save(&dir, &s)?;
            let nc = s.nonconformity(&id)?;
            render(
                output,
                "nonconformity raise",
                nc,
                format!(
                    "muster process revise <pid> \"<change>\" --because {id}  |  muster nonconformity resolve {id} --corrective-action \"<action>\""
                ),
            )
        }
        NonconformitySub::List => {
            let s = store::load(&dir)?;
            let items = s.list_nonconformities();
            let view = Listing::new(
                items,
                "nonconformity",
                "muster nonconformity show <id>".to_string(),
            );
            output.success("nonconformity list", &view, &mut io::stdout())?;
            Ok(())
        }
        NonconformitySub::Show { id } => {
            let s = store::load(&dir)?;
            let nc = s.nonconformity(&id)?;
            render(
                output,
                "nonconformity show",
                nc,
                "muster readiness".to_string(),
            )
        }
        NonconformitySub::Resolve {
            id,
            corrective_action,
        } => {
            let mut s = store::load(&dir)?;
            s.resolve_nonconformity(&id, corrective_action)?;
            store::save(&dir, &s)?;
            let nc = s.nonconformity(&id)?;
            render(
                output,
                "nonconformity resolve",
                nc,
                "muster readiness".to_string(),
            )
        }
    }
}

fn render(output: &Output, command: &str, nc: &domain::Nonconformity, next: String) -> Boxed {
    let view = WithNext::new(nc, next);
    output.success(command, &view, &mut io::stdout())?;
    Ok(())
}
