//! `readiness` command handler — the headline truth-meter.
//!
//! The cli owns the clock + resolution (#8): it projects every control to its
//! honest `Derived` state and bakes ref-backed check results to their derived
//! outcome, then hands both to the pure `domain::readiness_with`.

use crate::resolve;
use crate::root::ReadinessArgs;
use crate::store;
use crate::view::WithNext;
use domain::EvidenceVerdict;
use domain::reference::Derived;
use infrastructure::output::Output;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::Path;

type Boxed = Result<i32, Box<dyn std::error::Error>>;

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
    /// `true` when `MUSTER_CMD_CACHE` is on — command-ref verdicts are served from
    /// a cache and may be stale (b3). A machine flag so an agent surface can react;
    /// the human surface also prints a warning line.
    cmd_cache_mode: bool,
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
        if self.cmd_cache_mode {
            writeln!(f, "  {}", store::CMD_CACHE_WARNING)?;
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
    // honor-VERIFIED (b1, default-on): a `file` evidence counts only if the path
    // RESOLVES to an existing file (cwd-relative at read time, like `--ref-file`);
    // a `url` only if well-formed (FORMAT only — v1 is NO-NETWORK). The cli owns
    // the fs boundary (#8): it injects `Path::is_file` into the pure
    // `domain::verify_evidence`, which is `false` for a missing path AND a dir.
    let mut evidence_index: BTreeMap<String, EvidenceVerdict> = BTreeMap::new();
    let mut drift_profiles: Vec<DriftProfileEntry> = Vec::new();
    for c in s.controls.values() {
        evidence_index.insert(
            c.id.clone(),
            domain::verify_evidence(&c.evidence, |p| Path::new(p).is_file()),
        );
        let own = resolve::project(
            c.r#ref.as_ref(),
            c.resolved.as_ref(),
            &now,
            fresh,
            cmd_cache,
        );
        let own_opt = c.is_ref_backed().then(|| own.clone());
        let impls: Vec<Derived> = c
            .implementations
            .iter()
            .map(|im| {
                resolve::project(
                    Some(&im.r#ref),
                    im.resolved.as_ref(),
                    &now,
                    fresh,
                    cmd_cache,
                )
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
        } else if let Some(r) = c.implementations.first().map(|im| &im.r#ref) {
            drift_profiles.push(DriftProfileEntry {
                id: c.id.clone(),
                profile: domain::drift_profile(r, &projected, cmd_cache),
            });
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

    // honor-VERIFIED for the proven/asserted split (mirrors the control
    // `evidence_index` loop above): a process's verifying artifact counts toward
    // `proven` only if it RESOLVES (a `file` exists / a `url` is well-formed). The
    // cli owns the fs boundary (#8): inject `Path::is_file` into the pure
    // `domain::verify_evidence` (SSOT — same helper as control coverage).
    let mut process_evidence_index: BTreeMap<String, EvidenceVerdict> = BTreeMap::new();
    for p in s.processes.values() {
        process_evidence_index.insert(
            p.id.clone(),
            domain::verify_evidence(&p.evidence, |path| Path::new(path).is_file()),
        );
    }

    let result = domain::readiness_with(
        &s,
        args.process.as_deref(),
        &index,
        &evidence_index,
        &process_evidence_index,
        store::source_freshness_secs(),
    );
    // SSOT: the ready/not-ready decision lives in `Readiness::is_ready()` (domain),
    // reused here by BOTH the `next` hint and the `--require-ready` gate below.
    let next = if result.is_ready() {
        "you are certification-ready — keep evidence fresh".to_string()
    } else {
        "address a gap finding above, then re-run: muster readiness".to_string()
    };
    let readiness_view = ReadinessView {
        readiness: &result,
        drift_profiles,
        cmd_cache_mode: cmd_cache,
    };
    let view = WithNext::new(&readiness_view, next);
    // Render FIRST, gate SECOND: the full readiness output (human or JSON) is always
    // emitted regardless of the gate outcome — the exit code and the rendered output
    // are independent channels. A gate miss is a SUCCESSFUL computation that did not
    // meet the bar, never an error envelope.
    output.success("readiness", &view, &mut io::stdout())?;
    let code = if args.require_ready && !result.is_ready() {
        crate::EXIT_GATE_NOT_MET
    } else {
        crate::EXIT_OK
    };
    Ok(code)
}
