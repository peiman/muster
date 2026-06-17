//! `control` command handlers.
//!
//! Reference-backed controls derive their `title`/`status` from the pointed-at
//! source on read (#7). The cli owns the clock + resolution policy (`resolve`),
//! domain owns the pure projection (`display_title`/`effective_status`/`project`).

use crate::resolve;
use crate::root::ControlSub;
use crate::store;
use crate::view::{Listing, WithNext};
use domain::reference::Derived;
use domain::{Control, Evidence, Ref};
use infrastructure::output::Output;
use serde::Serialize;
use std::fmt;
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
            ref_flags,
        } => {
            let r = ref_flags.to_ref()?;
            let mut s = store::load(&dir)?;
            s.add_control(&id, &title, clause_ref, applicable.unwrap_or(true))?;
            if let Some(r) = r {
                s.set_control_ref(&id, r.clone())?;
                // Populate the cache once at creation so `command` refs have a
                // resolution to serve (file_anchor re-resolves live on read).
                let res = resolve::resolve(&r, &store::now_iso());
                s.set_control_resolution(&id, res)?;
            }
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            render_view(
                output,
                "control add",
                c,
                format!("muster control show {id}  |  muster process link-control <pid> {id}"),
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
            render_view(output, "control show", c, "muster readiness".to_string())
        }
        ControlSub::SetStatus { id, status } => {
            let mut s = store::load(&dir)?;
            s.set_control_status(&id, status)?;
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            render_view(
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
            render_view(
                output,
                "control attach-evidence",
                c,
                "muster readiness".to_string(),
            )
        }
        ControlSub::Resolve { id } => {
            let mut s = store::load(&dir)?;
            let now = store::now_iso();
            // Re-resolve the control's own ref + every implementation's ref.
            let c = s.control(&id)?.clone();
            if let Some(r) = &c.r#ref {
                let res = resolve::resolve(r, &now);
                s.set_control_resolution(&id, res)?;
            }
            for im in &c.implementations {
                let res = resolve::resolve(&im.r#ref, &now);
                s.set_implementation_resolution(&id, &im.id, res)?;
            }
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            render_view(output, "control resolve", c, "muster readiness".to_string())
        }
        ControlSub::AddImplementation {
            id,
            impl_id,
            ref_flags,
        } => {
            let r = ref_flags
                .to_ref()?
                .ok_or("an implementation needs a ref — pass --ref-file/--ref-anchor, --ref-cmd/--ref-dir, or --ref-note")?;
            let mut s = store::load(&dir)?;
            s.add_implementation(&id, &impl_id, r.clone())?;
            let res = resolve::resolve(&r, &store::now_iso());
            s.set_implementation_resolution(&id, &impl_id, res)?;
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            render_view(
                output,
                "control add-implementation",
                c,
                format!("muster control show {id}"),
            )
        }
        ControlSub::Import {
            manifest,
            format,
            prefix,
            title_field,
        } => crate::import::execute(&dir, &manifest, format, &prefix, &title_field, output),
    }
}

// ── rich view: derived title/status + honest resolution state ─────────────────

#[derive(Serialize)]
struct ImplView<'a> {
    id: &'a str,
    r#ref: &'a Ref,
    resolution: Derived,
}

/// The dual-surface projection of a control on read: `title`/`status` are the
/// DERIVED values; the stored title surfaces as `fallback_title` and the stored
/// status as `asserted_status` only when a ref/implementations override them
/// (#7). `resolution` is the honest `resolution_state` of the control's own ref.
#[derive(Serialize)]
struct ControlView<'a> {
    id: &'a str,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    clause_ref: &'a Option<String>,
    applicable: bool,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    asserted_status: Option<String>,
    evidence: &'a [Evidence],
    #[serde(skip_serializing_if = "Option::is_none")]
    r#ref: &'a Option<Ref>,
    resolution: Derived,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    implementations: Vec<ImplView<'a>>,
}

impl ControlView<'_> {
    fn build(c: &Control) -> ControlView<'_> {
        let now = store::now_iso();
        let fresh = store::freshness_secs();
        let own = resolve::project(c.r#ref.as_ref(), c.resolved.as_ref(), &now, fresh);
        let impl_views: Vec<ImplView> = c
            .implementations
            .iter()
            .map(|im| ImplView {
                id: &im.id,
                r#ref: &im.r#ref,
                resolution: resolve::project(Some(&im.r#ref), im.resolved.as_ref(), &now, fresh),
            })
            .collect();
        let impl_deriveds: Vec<Derived> = impl_views.iter().map(|v| v.resolution.clone()).collect();

        let derived = c.is_ref_backed() || !c.implementations.is_empty();
        let own_opt = c.is_ref_backed().then_some(&own);
        let title = c.display_title(Some(&own));
        let status = c.effective_status(own_opt, &impl_deriveds);

        ControlView {
            id: &c.id,
            title,
            fallback_title: c.is_ref_backed().then(|| c.title.clone()),
            clause_ref: &c.clause_ref,
            applicable: c.applicable,
            status: status.to_string(),
            asserted_status: derived.then(|| c.status.to_string()),
            evidence: &c.evidence,
            r#ref: &c.r#ref,
            resolution: own,
            implementations: impl_views,
        }
    }
}

fn resolution_label(d: &Derived) -> String {
    match d {
        Derived::Derived { value, outcome, .. } => format!("derived: {value} ({outcome:?})"),
        Derived::Stale {
            value,
            outcome,
            resolved_ts,
        } => format!("stale (resolved {resolved_ts}): {value} ({outcome:?})"),
        Derived::Unresolved { reason } => format!("unresolved: {reason}"),
        Derived::Asserted => "asserted (unverified)".to_string(),
    }
}

impl fmt::Display for ControlView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "control {} — {}", self.id, self.title)?;
        writeln!(f, "  status: {}", self.status)?;
        writeln!(f, "  applicable: {}", self.applicable)?;
        if let Some(fb) = &self.fallback_title {
            writeln!(f, "  fallback_title: {fb}")?;
        }
        if let Some(cr) = &self.clause_ref {
            writeln!(f, "  clause_ref: {cr}")?;
        }
        if self.r#ref.is_some() {
            writeln!(f, "  resolution: {}", resolution_label(&self.resolution))?;
        }
        if !self.evidence.is_empty() {
            writeln!(f, "  evidence:")?;
            for e in self.evidence {
                writeln!(f, "    {e}")?;
            }
        }
        if !self.implementations.is_empty() {
            writeln!(f, "  implementations:")?;
            for im in &self.implementations {
                writeln!(f, "    {} — {}", im.id, resolution_label(&im.resolution))?;
            }
        }
        Ok(())
    }
}

fn render_view(output: &Output, command: &str, control: &Control, next: String) -> Boxed {
    let view = ControlView::build(control);
    let wrapped = WithNext::new(&view, next);
    output.success(command, &wrapped, &mut io::stdout())?;
    Ok(())
}
