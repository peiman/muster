//! `process` command handlers — the spine. Pattern: resolve data dir → load →
//! pure domain op (passing `ts` from the boundary clock) → save → render.

use crate::root::{CheckArgs, CheckSub, MetricSub, ProcessSub, RiskSub, StepSub};
use crate::store;
use crate::view::{Listing, WithNext};
use domain::{CheckResult, Evidence, EvidenceKind};
use infrastructure::output::Output;
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
                render(output, "process show", p, "muster readiness".to_string())
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
    // Create form: `process check add <id> --description --enforcement`.
    if let Some(CheckSub::Add {
        id,
        description,
        enforcement,
    }) = args.sub
    {
        let mut s = store::load(dir)?;
        let check_id = s.add_check(&id, &description, enforcement)?;
        store::save(dir, &s)?;
        let p = s.process(&id)?;
        return render(
            output,
            "process check add",
            p,
            format!("muster process check {id} {check_id} --pass"),
        );
    }

    // Ingest form: `process check <id> <check-id> --pass|--fail [--evidence k v]`.
    let id = args
        .id
        .ok_or("missing process id — usage: muster process check <id> <check-id> --pass|--fail")?;
    let check_id = args
        .check_id
        .ok_or("missing check id — usage: muster process check <id> <check-id> --pass|--fail")?;
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
    s.ingest_check(&id, &check_id, result, &ts, evidence)?;
    store::save(dir, &s)?;
    let p = s.process(&id)?;
    render(output, "process check", p, "muster readiness".to_string())
}

fn render(output: &Output, command: &str, process: &domain::Process, next: String) -> Boxed {
    let view = WithNext::new(process, next);
    output.success(command, &view, &mut io::stdout())?;
    Ok(())
}
