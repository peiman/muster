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
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::io;

type Boxed = Result<(), Box<dyn std::error::Error>>;

/// One control's ref-kind drift profile (SC-3): the honesty risk of its weakest
/// link, drawn from the fixed set `live_resolved | cached_command | stale |
/// unresolved | asserted`. id-sorted in the readiness output (deterministic, AX).
#[derive(Serialize)]
struct DriftProfileEntry {
    id: String,
    profile: &'static str,
}

/// The cli-side readiness view: the pure `domain::Readiness` plus the per-control
/// ref-kind drift profile (which needs the ref kind + cache mode the cli owns).
/// `#[serde(flatten)]` keeps every existing readiness field at the top level so
/// the JSON surface is additive (no regression), with `drift_profiles` alongside.
#[derive(Serialize)]
struct ReadinessView<'a> {
    #[serde(flatten)]
    readiness: &'a domain::Readiness,
    drift_profiles: Vec<DriftProfileEntry>,
}

impl fmt::Display for ReadinessView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.readiness)?;
        if !self.drift_profiles.is_empty() {
            writeln!(f, "  drift profile (ref-kind honesty):")?;
            for e in &self.drift_profiles {
                writeln!(f, "    {} — {}", e.id, e.profile)?;
            }
        }
        Ok(())
    }
}

pub fn execute(args: ReadinessArgs, output: &Output) -> Boxed {
    let dir = store::data_dir();
    let mut s = store::load(&dir)?;
    // Honest not-found if scoping to a process that doesn't exist.
    if let Some(pid) = &args.process {
        s.process(pid)?;
    }

    let now = store::now_iso();
    let fresh = store::freshness_secs();
    let cmd_cache = store::cmd_cache_enabled();

    // Build the control resolution index (id → honest Derived projection) and the
    // ref-kind drift profile per ref-backed control (SC-3: the weakest links are
    // visible). The profile is the domain's pure mapping (SSOT).
    let mut index: BTreeMap<String, Derived> = BTreeMap::new();
    let mut drift_profiles: Vec<DriftProfileEntry> = Vec::new();
    for c in s.controls.values() {
        let own = resolve::project(c.r#ref.as_ref(), c.resolved.as_ref(), &now, fresh, cmd_cache);
        let own_opt = c.is_ref_backed().then(|| own.clone());
        let impls: Vec<Derived> = c
            .implementations
            .iter()
            .map(|im| {
                resolve::project(Some(&im.r#ref), im.resolved.as_ref(), &now, fresh, cmd_cache)
            })
            .collect();
        let projected = c.project(own_opt, impls);
        // Drift profile: classify the control's own ref (the honesty anchor). A
        // control with only implementations is profiled by its worst projection.
        if let Some(r) = c.r#ref.as_ref() {
            drift_profiles.push(DriftProfileEntry {
                id: c.id.clone(),
                profile: domain::drift_profile(r, &projected, cmd_cache),
            });
        } else if !c.implementations.is_empty() {
            if let Some(r) = c.implementations.first().map(|im| &im.r#ref) {
                drift_profiles.push(DriftProfileEntry {
                    id: c.id.clone(),
                    profile: domain::drift_profile(r, &projected, cmd_cache),
                });
            }
        }
        index.insert(c.id.clone(), projected);
    }
    // Deterministic ordering (id-sorted).
    drift_profiles.sort_by(|a, b| a.id.cmp(&b.id));

    // Bake derived check results into the store so the existing refuting-signal /
    // failed-check logic sees the honest (resolved) outcome, not the stored one.
    for p in s.processes.values_mut() {
        for check in &mut p.checks {
            if check.is_ref_backed() {
                let d = resolve::project(
                    check.r#ref.as_ref(),
                    check.resolved.as_ref(),
                    &now,
                    fresh,
                    cmd_cache,
                );
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
    let readiness_view = ReadinessView {
        readiness: &result,
        drift_profiles,
    };
    let view = WithNext::new(&readiness_view, next);
    output.success("readiness", &view, &mut io::stdout())?;
    Ok(())
}
