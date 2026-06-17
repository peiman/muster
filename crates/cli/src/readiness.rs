//! `readiness` command handler — the headline truth-meter.
//!
//! The cli owns the clock + resolution (#8): it projects every control to its
//! honest `Derived` state and bakes ref-backed check results to their derived
//! outcome, then hands both to the pure `domain::readiness_with`.

use crate::resolve;
use crate::root::ReadinessArgs;
use crate::store;
use crate::view::WithNext;
use domain::reference::Derived;
use infrastructure::output::Output;
use std::collections::BTreeMap;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

pub fn execute(args: ReadinessArgs, output: &Output) -> Boxed {
    let dir = store::data_dir();
    let mut s = store::load(&dir)?;
    // Honest not-found if scoping to a process that doesn't exist.
    if let Some(pid) = &args.process {
        s.process(pid)?;
    }

    let now = store::now_iso();
    let fresh = store::freshness_secs();

    // Build the control resolution index (id → honest Derived projection).
    let mut index: BTreeMap<String, Derived> = BTreeMap::new();
    for c in s.controls.values() {
        let own = resolve::project(c.r#ref.as_ref(), c.resolved.as_ref(), &now, fresh);
        let own_opt = c.is_ref_backed().then(|| own.clone());
        let impls: Vec<Derived> = c
            .implementations
            .iter()
            .map(|im| resolve::project(Some(&im.r#ref), im.resolved.as_ref(), &now, fresh))
            .collect();
        index.insert(c.id.clone(), c.project(own_opt, impls));
    }

    // Bake derived check results into the store so the existing refuting-signal /
    // failed-check logic sees the honest (resolved) outcome, not the stored one.
    for p in s.processes.values_mut() {
        for check in &mut p.checks {
            if check.is_ref_backed() {
                let d =
                    resolve::project(check.r#ref.as_ref(), check.resolved.as_ref(), &now, fresh);
                check.last_result = check.effective_result(Some(&d));
            }
        }
    }

    let result = domain::readiness_with(&s, args.process.as_deref(), &index);
    let next = if result.verdict == "READY" {
        "you are certification-ready — keep evidence fresh".to_string()
    } else {
        "address a gap finding above, then re-run: muster readiness".to_string()
    };
    let view = WithNext::new(&result, next);
    output.success("readiness", &view, &mut io::stdout())?;
    Ok(())
}
