//! `control` command handlers.
//!
//! Reference-backed controls derive their `title`/`status` from the pointed-at
//! source on read (#7). The cli owns the clock + resolution policy (`resolve`),
//! domain owns the pure projection (`display_title`/`effective_status`/`project`).

use crate::resolve;
use crate::root::ControlSub;
use crate::store;
use crate::view::{Listing, WithNext};
use domain::reference::{Derived, Resolution};
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
            // Fail-closed anchor validation (#9): a file_anchor whose dotted anchor
            // does not resolve at store time is REFUSED before any persist, naming
            // the fix — no control is created (SC-5). A command ref's non-zero exit
            // is a legitimate *fail*, not a store error; we only refuse a command
            // that cannot be resolved at all (spawn failure ⇒ Unresolved).
            validate_ref_at_store_time(&r, &id)?;
            // The title is a DERIVED projection when a ref backs the control (#7),
            // so `--title` is optional then — fall back to the id as a legible
            // placeholder. An asserted (no-ref) control still needs a human label.
            let title =
                match (title, &r) {
                    (Some(t), _) => t,
                    (None, Some(_)) => id.clone(),
                    (None, None) => return Err(
                        "an asserted control needs --title (or back it with --ref-* to derive one)"
                            .into(),
                    ),
                };
            let mut s = store::load(&dir)?;
            s.add_control(&id, &title, clause_ref, applicable.unwrap_or(true))?;
            let steering_notice = steering_notice_for(r.as_ref());
            if let Some(r) = r {
                s.set_control_ref(&id, r.clone())?;
                persist_resolution_if_cached(&mut s, &id, &r)?;
            }
            store::save(&dir, &s)?;
            let c = s.control(&id)?;
            let mut view = ControlView::build(c);
            view.steering_notice = steering_notice;
            let wrapped = WithNext::new(
                &view,
                format!("muster control show {id}  |  muster process link-control <pid> {id}"),
            );
            output.success("control add", &wrapped, &mut io::stdout())?;
            Ok(())
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
        ControlSub::Resolve { id, all } => match (id, all) {
            (Some(_), true) => {
                Err("pass either a control <id> or --all, not both — usage: muster control resolve <id> | muster control resolve --all".into())
            }
            (None, false) => {
                Err("name a control to resolve, or pass --all — usage: muster control resolve <id> | muster control resolve --all".into())
            }
            (Some(id), false) => {
                let mut s = store::load(&dir)?;
                // Re-resolve the control's own ref + every implementation's ref.
                // In the honest default this only matters in opt-in cache mode (the
                // read path re-resolves live regardless); the refreshed view below
                // reflects the live projection either way.
                let c = s.control(&id)?.clone();
                if let Some(r) = &c.r#ref {
                    persist_resolution_if_cached(&mut s, &id, r)?;
                }
                for im in &c.implementations {
                    persist_impl_resolution_if_cached(&mut s, &id, &im.id, &im.r#ref)?;
                }
                store::save(&dir, &s)?;
                let c = s.control(&id)?;
                render_view(output, "control resolve", c, "muster readiness".to_string())
            }
            (None, true) => resolve_all(&dir, output),
        },
        ControlSub::AddImplementation {
            id,
            impl_id,
            ref_flags,
        } => {
            let r = ref_flags
                .to_ref()?
                .ok_or("an implementation needs a ref — pass --ref-file/--ref-anchor, --ref-cmd/--ref-dir, --ref-report, or --ref-note")?;
            // Fail-closed anchor validation (#9), mirroring `control add`.
            validate_ref_at_store_time(&Some(r.clone()), &id)?;
            let mut s = store::load(&dir)?;
            s.add_implementation(&id, &impl_id, r.clone())?;
            persist_impl_resolution_if_cached(&mut s, &id, &impl_id, &r)?;
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
    /// A one-line steering notice (SC-3): set only on `control add` with a
    /// `--ref-cmd`, recommending the zero-drift `--ref-report` path. Renders in
    /// BOTH surfaces (no human-only data).
    #[serde(skip_serializing_if = "Option::is_none")]
    steering_notice: Option<String>,
}

impl ControlView<'_> {
    fn build(c: &Control) -> ControlView<'_> {
        let now = store::now_iso();
        let fresh = store::freshness_secs();
        let cmd_cache = store::cmd_cache_enabled();
        let own = resolve::project(
            c.r#ref.as_ref(),
            c.resolved.as_ref(),
            &now,
            fresh,
            cmd_cache,
        );
        let impl_views: Vec<ImplView> = c
            .implementations
            .iter()
            .map(|im| ImplView {
                id: &im.id,
                r#ref: &im.r#ref,
                resolution: resolve::project(
                    Some(&im.r#ref),
                    im.resolved.as_ref(),
                    &now,
                    fresh,
                    cmd_cache,
                ),
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
            steering_notice: None,
        }
    }
}

fn resolution_label(d: &Derived) -> String {
    match d {
        Derived::Derived {
            value,
            outcome,
            resolved_age_secs,
            served_from_cache,
            source_age_secs,
            ..
        } => {
            let cache = if *served_from_cache { "cached" } else { "live" };
            let src = source_age_secs
                .map(|a| format!(" src_age={a}s"))
                .unwrap_or_default();
            format!("derived: {value} ({outcome:?}) age={resolved_age_secs}s {cache}{src}")
        }
        Derived::Stale {
            value,
            outcome,
            resolved_ts,
            resolved_age_secs,
            served_from_cache,
        } => {
            let cache = if *served_from_cache { "cached" } else { "live" };
            format!(
                "stale (resolved {resolved_ts}): {value} ({outcome:?}) age={resolved_age_secs}s {cache}"
            )
        }
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
        if let Some(notice) = &self.steering_notice {
            writeln!(f, "  notice: {notice}")?;
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

// ── store-time validation + steering + cache helpers ──────────────────────────

/// Fail-closed anchor validation (#9). A `file_anchor` whose dotted anchor does
/// not resolve at store time is refused, naming the path/anchor + reason, so no
/// dangling control is ever persisted (SC-5). Command refs are NOT refused on a
/// non-zero exit (a legitimate fail), only on an outright spawn failure
/// (`Unresolved`). Notes / no-ref are always allowed.
fn validate_ref_at_store_time(r: &Option<Ref>, id: &str) -> Result<(), Box<dyn std::error::Error>> {
    match r {
        Some(rf @ Ref::FileAnchor { path, anchor, .. }) => {
            if let Resolution::Unresolved { reason } = resolve::resolve(rf, &store::now_iso()) {
                return Err(format!(
                    "refusing to add '{id}': the file_anchor does not resolve — {reason}. Fix the source file '{path}' or the anchor '{anchor}', then retry (no control was created)."
                )
                .into());
            }
            Ok(())
        }
        Some(rc @ Ref::Command { cmd, .. }) => {
            if let Resolution::Unresolved { reason } = resolve::resolve(rc, &store::now_iso()) {
                return Err(format!(
                    "refusing to add '{id}': the command ref could not run — {reason}. Fix the command '{cmd}' or its --ref-dir, then retry (no control was created)."
                )
                .into());
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The one-line steering notice for `control add` (SC-3): only for a `command`
/// ref, recommending the zero-drift report path. `None` for every other ref.
fn steering_notice_for(r: Option<&Ref>) -> Option<String> {
    matches!(r, Some(Ref::Command { .. })).then(|| {
        "--ref-cmd re-runs a command on every read; prefer --ref-report <artifact> <anchor> when the tool emits a result file — zero drift, no re-run.".to_string()
    })
}

/// Persist a control's resolution ONLY in opt-in command-cache mode (#7 SSOT: no
/// authoritative stored copy for live refs). In the honest default the control's
/// ref re-resolves live on read, so no `resolved` copy is written.
fn persist_resolution_if_cached(
    s: &mut domain::Store,
    id: &str,
    r: &Ref,
) -> Result<(), Box<dyn std::error::Error>> {
    if store::cmd_cache_enabled() && r.is_cached_kind() {
        let res = resolve::resolve(r, &store::now_iso());
        s.set_control_resolution(id, res)?;
    }
    Ok(())
}

/// Implementation counterpart to [`persist_resolution_if_cached`].
fn persist_impl_resolution_if_cached(
    s: &mut domain::Store,
    cid: &str,
    impl_id: &str,
    r: &Ref,
) -> Result<(), Box<dyn std::error::Error>> {
    if store::cmd_cache_enabled() && r.is_cached_kind() {
        let res = resolve::resolve(r, &store::now_iso());
        s.set_implementation_resolution(cid, impl_id, res)?;
    }
    Ok(())
}

// ── control resolve --all (the doctor surface, SC-5) ──────────────────────────

#[derive(Serialize)]
struct ResolveEntry {
    id: String,
    resolution_state: String,
    /// `true` when the control's own ref silently went `Unresolved` (e.g. after a
    /// source refactor) — the flag a doctor sweep surfaces.
    unresolved: bool,
}

#[derive(Serialize)]
struct ResolveAllReport {
    resolved: Vec<ResolveEntry>,
    unresolved_count: usize,
}

impl fmt::Display for ResolveAllReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "re-resolved {} ref-backed control(s); {} unresolved",
            self.resolved.len(),
            self.unresolved_count
        )?;
        for e in &self.resolved {
            let flag = if e.unresolved { "  ⚠ UNRESOLVED" } else { "" };
            writeln!(f, "  {} — {}{}", e.id, e.resolution_state, flag)?;
        }
        Ok(())
    }
}

fn resolution_state_str(d: &Derived) -> &'static str {
    match d {
        Derived::Derived { .. } => "derived",
        Derived::Stale { .. } => "stale",
        Derived::Unresolved { .. } => "unresolved",
        Derived::Asserted => "asserted",
    }
}

/// Re-resolve every ref-backed control + implementation and emit a report that
/// flags any control whose own ref is now `Unresolved` (#1: surface silent rot).
/// In opt-in cache mode this also refreshes the persisted command-ref caches.
fn resolve_all(dir: &std::path::Path, output: &Output) -> Boxed {
    let mut s = store::load(dir)?;
    let now = store::now_iso();
    let fresh = store::freshness_secs();
    let cmd_cache = store::cmd_cache_enabled();

    // Refresh persisted caches first (cache mode only), then project live.
    let ids: Vec<String> = s.list_controls().iter().map(|c| c.id.clone()).collect();
    for id in &ids {
        let c = s.control(id)?.clone();
        if let Some(r) = &c.r#ref {
            persist_resolution_if_cached(&mut s, id, r)?;
        }
        for im in &c.implementations {
            persist_impl_resolution_if_cached(&mut s, id, &im.id, &im.r#ref)?;
        }
    }
    store::save(dir, &s)?;

    let mut entries: Vec<ResolveEntry> = Vec::new();
    let mut unresolved_count = 0usize;
    for id in &ids {
        let c = s.control(id)?;
        if !c.is_ref_backed() && c.implementations.is_empty() {
            continue; // asserted controls have no ref to re-resolve.
        }
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
        let state = resolution_state_str(&projected);
        let unresolved = state == "unresolved";
        if unresolved {
            unresolved_count += 1;
        }
        entries.push(ResolveEntry {
            id: id.clone(),
            resolution_state: state.to_string(),
            unresolved,
        });
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));

    let report = ResolveAllReport {
        resolved: entries,
        unresolved_count,
    };
    let next = if unresolved_count == 0 {
        "all refs resolve — run: muster readiness".to_string()
    } else {
        "fix the ⚠ UNRESOLVED sources/anchors above, then: muster control resolve --all".to_string()
    };
    let wrapped = WithNext::new(&report, next);
    output.success("control resolve", &wrapped, &mut io::stdout())?;
    Ok(())
}
