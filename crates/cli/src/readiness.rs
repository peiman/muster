//! `readiness` command handler — the headline truth-meter.

use crate::root::ReadinessArgs;
use crate::store;
use crate::view::WithNext;
use infrastructure::output::Output;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

pub fn execute(args: ReadinessArgs, output: &Output) -> Boxed {
    let dir = store::data_dir();
    let s = store::load(&dir)?;
    // Honest not-found if scoping to a process that doesn't exist.
    if let Some(pid) = &args.process {
        s.process(pid)?;
    }
    let result = domain::readiness(&s, args.process.as_deref());
    let next = if result.verdict == "READY" {
        "you are certification-ready — keep evidence fresh".to_string()
    } else {
        "address a gap finding above, then re-run: muster readiness".to_string()
    };
    let view = WithNext::new(&result, next);
    output.success("readiness", &view, &mut io::stdout())?;
    Ok(())
}
