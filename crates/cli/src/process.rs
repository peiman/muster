//! `process` command handlers — the spine. Pattern: resolve data dir → load →
//! pure domain op (passing `ts` from the boundary clock) → save → render.

use crate::resolve;
use crate::root::{CheckArgs, CheckSub, MetricSub, ProcessSub, RiskSub, StepSub};
use crate::store;
use crate::view::{Listing, WithNext};
use domain::{CheckResult, Evidence, EvidenceKind, Process};
use infrastructure::output::Output;
use serde::{Serialize, Serializer};
use std::fmt;
use std::io;
use std::str::FromStr;

type Boxed = Result<(), Box<dyn std::error::Error>>;

pub fn execute(sub: ProcessSub, output: &Output) -> Boxed {
    let dir = store::data_dir();
    match sub {
        ProcessSub::Add {
            id,
            name,
            owner,
            purpose,
        } => {
            let mut s = store::load(&dir)?;
            s.add_process(&id, &name, owner, purpose)?;
            store::save(&dir, &s)?;
            let p = s.process(&id)?;
            render(
                output,
                "process add",
                p,
                format!(
                    "muster process step add {id} --description <d>  |  muster process set-status {id} active"
                ),
            )
        }
        ProcessSub::Show { id, tree } => {
            let s = store::load(&dir)?;
            if tree {
                let t = s.show_tree(&id)?;
                let view = WithNext::new(&t, "muster readiness".to_string());
                output.success("process show", &view, &mut io::stdout())?;
                Ok(())
            } else {
                let p = s.process(&id)?;
                let view = ProcessShow::build(p);
                let wrapped = WithNext::new(&view, "muster readiness".to_string());
                output.success("process show", &wrapped, &mut io::stdout())?;
                Ok(())
            }
        }
        ProcessSub::List => {
            let s = store::load(&dir)?;
            let items = s.list_processes();
            let view = Listing::new(items, "process", "muster process show <id>".to_string());
            output.success("process list", &view, &mut io::stdout())?;
            Ok(())
        }
        ProcessSub::SetStatus { id, status } => {
            let mut s = store::load(&dir)?;
            s.set_process_status(&id, status)?;
            store::save(&dir, &s)?;
            let p = s.process(&id)?;
            render(
                output,
                "process set-status",
                p,
                "muster readiness".to_string(),
            )
        }
        ProcessSub::Step(cmd) => match cmd.sub {
            StepSub::Add {
                id,
                description,
                owner,
                control,
                process_ref,
            } => {
                let mut s = store::load(&dir)?;
                s.add_step(&id, &description, owner, control, process_ref)?;
                store::save(&dir, &s)?;
                let p = s.process(&id)?;
                render(
                    output,
                    "process step add",
                    p,
                    format!("muster process show {id} --tree"),
                )
            }
        },
        ProcessSub::LinkControl { id, control_id } => {
            let mut s = store::load(&dir)?;
            s.link_control(&id, &control_id)?;
            store::save(&dir, &s)?;
            let p = s.process(&id)?;
            render(
                output,
                "process link-control",
                p,
                format!("muster control set-status {control_id} implemented"),
            )
        }
        ProcessSub::Risk(cmd) => match cmd.sub {
            RiskSub::Add { id, risk } => {
                let mut s = store::load(&dir)?;
                s.add_risk(&id, &risk)?;
                store::save(&dir, &s)?;
                let p = s.process(&id)?;
                render(
                    output,
                    "process risk add",
                    p,
                    "muster readiness".to_string(),
                )
            }
        },
        ProcessSub::Metric(cmd) => match cmd.sub {
            MetricSub::Add { id, metric } => {
                let mut s = store::load(&dir)?;
                s.add_metric(&id, &metric)?;
                store::save(&dir, &s)?;
                let p = s.process(&id)?;
                render(
                    output,
                    "process metric add",
                    p,
                    "muster readiness".to_string(),
                )
            }
        },
        ProcessSub::Check(args) => check(args, &dir, output),
        ProcessSub::Revise {
            id,
            summary,
            because,
        } => {
            let mut s = store::load(&dir)?;
            let ts = store::now_iso();
            s.revise(&id, &summary, because, &ts)?;
            store::save(&dir, &s)?;
            let p = s.process(&id)?;
            render(
                output,
                "process revise",
                p,
                format!("muster process show {id}"),
            )
        }
        ProcessSub::AttachEvidence { id, kind, value } => {
            let mut s = store::load(&dir)?;
            s.attach_process_evidence(&id, Evidence { kind, value })?;
            store::save(&dir, &s)?;
            let p = s.process(&id)?;
            render(
                output,
                "process attach-evidence",
                p,
                "muster readiness".to_string(),
            )
        }
    }
}

fn check(args: CheckArgs, dir: &std::path::Path, output: &Output) -> Boxed {
    // Create form: `process check add <id> --description --enforcement [--ref-*]`.
    if let Some(CheckSub::Add {
        id,
        description,
        enforcement,
        ref_flags,
    }) = args.sub
    {
        let r = ref_flags.to_ref()?;
        let mut s = store::load(dir)?;
        let check_id = s.add_check(&id, &description, enforcement)?;
        let next = if let Some(r) = r {
            s.set_check_ref(&id, &check_id, r.clone())?;
            // #7 SSOT: persist a cached resolution only in opt-in command-cache
            // mode; otherwise the check re-resolves live on read (no stale green).
            if store::cmd_cache_enabled() && r.is_cached_kind() {
                let res = resolve::resolve(&r, &store::now_iso());
                s.set_check_resolution(&id, &check_id, res)?;
            }
            // A ref-backed check derives its result; it cannot be hand-set.
            format!("muster process check {id} {check_id} --resolve")
        } else {
            format!("muster process check {id} {check_id} --pass")
        };
        store::save(dir, &s)?;
        let p = s.process(&id)?;
        return render(output, "process check add", p, next);
    }

    // Ingest / resolve form: `process check <id> <check-id> --pass|--fail|--resolve`.
    let id = args.id.ok_or(
        "missing process id — usage: muster process check <id> <check-id> --pass|--fail|--resolve",
    )?;
    let check_id = args.check_id.ok_or(
        "missing check id — usage: muster process check <id> <check-id> --pass|--fail|--resolve",
    )?;

    // Resolve form: re-run the check's ref and refresh its cache.
    if args.resolve {
        let mut s = store::load(dir)?;
        let p = s.process(&id)?;
        let c = p
            .checks
            .iter()
            .find(|c| c.id == check_id)
            .ok_or_else(|| format!("check '{check_id}' not found on process '{id}'"))?;
        let r = c
            .r#ref
            .clone()
            .ok_or_else(|| format!("check '{check_id}' has no ref to resolve"))?;
        // #7 SSOT: only opt-in command-cache mode keeps a stored copy; live refs
        // re-resolve on read so the refreshed view below is already authoritative.
        if store::cmd_cache_enabled() && r.is_cached_kind() {
            let res = resolve::resolve(&r, &store::now_iso());
            s.set_check_resolution(&id, &check_id, res)?;
        }
        store::save(dir, &s)?;
        let p = s.process(&id)?;
        return render(output, "process check", p, "muster readiness".to_string());
    }

    let result = match (args.pass, args.fail) {
        (true, false) => CheckResult::Pass,
        (false, true) => CheckResult::Fail,
        _ => {
            return Err("a conformance result must be exactly one of --pass or --fail".into());
        }
    };
    let evidence = match &args.evidence {
        Some(kv) if kv.len() == 2 => Some(Evidence {
            kind: EvidenceKind::from_str(&kv[0])?,
            value: kv[1].clone(),
        }),
        _ => None,
    };
    let mut s = store::load(dir)?;
    let ts = store::now_iso();
    // Ref-backed checks reject hand-set results (honesty rule, SC-5).
    s.ingest_check(&id, &check_id, result, &ts, evidence)?;
    store::save(dir, &s)?;
    let p = s.process(&id)?;
    render(output, "process check", p, "muster readiness".to_string())
}

fn render(output: &Output, command: &str, process: &Process, next: String) -> Boxed {
    let view = ProcessShow::build(process);
    let wrapped = WithNext::new(&view, next);
    output.success(command, &wrapped, &mut io::stdout())?;
    Ok(())
}

/// `process show` view: identical to the raw `Process` JSON, except each
/// ref-backed check's `last_result` is the DERIVED outcome (resolved on read,
/// SC-4) and carries an honest `resolution` field. The human surface renders a
/// clone with derived results so text and JSON tell the same story (#7).
struct ProcessShow {
    json: serde_json::Value,
    display_clone: Process,
}

impl ProcessShow {
    fn build(p: &Process) -> ProcessShow {
        let now = store::now_iso();
        let fresh = store::freshness_secs();
        let cmd_cache = store::cmd_cache_enabled();

        // A clone whose ref-backed checks show their derived result (human surface).
        let mut display_clone = p.clone();
        for c in &mut display_clone.checks {
            if c.is_ref_backed() {
                let derived =
                    resolve::project(c.r#ref.as_ref(), c.resolved.as_ref(), &now, fresh, cmd_cache);
                c.last_result = c.effective_result(Some(&derived));
            }
        }

        // JSON from the derived clone, then splice in each check's resolution.
        let mut json = serde_json::to_value(&display_clone).unwrap_or(serde_json::Value::Null);
        if let Some(arr) = json.get_mut("checks").and_then(|v| v.as_array_mut()) {
            for (cv, c) in arr.iter_mut().zip(p.checks.iter()) {
                if c.is_ref_backed() {
                    let derived = resolve::project(
                        c.r#ref.as_ref(),
                        c.resolved.as_ref(),
                        &now,
                        fresh,
                        cmd_cache,
                    );
                    if let Ok(rv) = serde_json::to_value(&derived)
                        && let Some(obj) = cv.as_object_mut()
                    {
                        obj.insert("resolution".to_string(), rv);
                    }
                }
            }
        }

        ProcessShow {
            json,
            display_clone,
        }
    }
}

impl Serialize for ProcessShow {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.json.serialize(s)
    }
}

impl fmt::Display for ProcessShow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_clone)
    }
}
