//! The cli bridge — the only place `domain` and `infrastructure` meet (#8).
//!
//! domain defines the pure `Ref`/`Resolution`/`Derived` types but cannot do I/O;
//! infrastructure does the I/O but cannot see domain. This module owns the clock
//! and the resolution policy: it calls the right infra resolver, maps the infra
//! result to a `domain::Resolution`, and projects it to the honest `Derived`
//! display state — re-resolving BOTH `file_anchor` AND (by default) `command`
//! refs live on every read, so a passed verdict can never outlive reality
//! (Manifesto #9: the drift window is structurally closed). Only the opt-in
//! `MUSTER_CMD_CACHE` mode serves a stored command verdict, which then goes
//! `Stale` past the freshness bound. Every resolved projection carries its
//! freshness evidence (`resolved_age_secs` / `served_from_cache`, plus the source
//! artifact's age for `file_anchor`).

use crate::store;
use domain::reference::{Derived, Outcome, Ref, Resolution};
use infrastructure::resolver;

/// Dereference a `Ref` into a `domain::Resolution` via live I/O. `now_iso` is the
/// read clock (the domain stays clock-free, C6).
pub fn resolve(r: &Ref, now_iso: &str) -> Resolution {
    resolve_meta(r, now_iso).0
}

/// Like [`resolve`] but also returns the source artifact's mtime (epoch seconds)
/// for `file_anchor` refs — the cli computes the source *age* against its clock
/// (#1 Truth-Seeking: surface how fresh the pointed-at artifact is). `None` for
/// command/note refs (no single source file) and for unresolved file anchors.
fn resolve_meta(r: &Ref, now_iso: &str) -> (Resolution, Option<i64>) {
    match r {
        Ref::FileAnchor { path, anchor } => {
            let fr = resolver::resolve_file_anchor(path, anchor);
            let mtime = fr.source_mtime_epoch;
            match fr.value {
                Some(value) => {
                    let outcome = domain::value_to_outcome(&value);
                    (
                        Resolution::Resolved {
                            value,
                            outcome,
                            resolved_ts: now_iso.to_string(),
                            source_excerpt: fr.excerpt,
                        },
                        mtime,
                    )
                }
                None => (
                    Resolution::Unresolved {
                        reason: fr.reason.unwrap_or_else(|| "unresolved".to_string()),
                    },
                    mtime,
                ),
            }
        }
        Ref::Command { cmd, dir } => (resolve_command(cmd, dir, now_iso), None),
        // A note is manual — it produces a value but no honesty claim. The
        // projection surfaces it as `Asserted`.
        Ref::Note { text } => (
            Resolution::Resolved {
                value: text.clone(),
                outcome: Outcome::Unknown,
                resolved_ts: now_iso.to_string(),
                source_excerpt: None,
            },
            None,
        ),
    }
}

fn resolve_command(cmd: &str, dir: &str, now_iso: &str) -> Resolution {
    let cr = resolver::run_command(cmd, dir);
    match cr.exit_code {
        Some(0) => Resolution::Resolved {
            value: "pass".to_string(),
            outcome: Outcome::Pass,
            resolved_ts: now_iso.to_string(),
            source_excerpt: tail_excerpt(&cr.stdout_tail),
        },
        Some(code) => Resolution::Resolved {
            value: format!("exit {code}"),
            outcome: Outcome::Fail,
            resolved_ts: now_iso.to_string(),
            source_excerpt: tail_excerpt(&cr.stdout_tail),
        },
        None => Resolution::Unresolved {
            reason: cr
                .reason
                .unwrap_or_else(|| "command spawn failed".to_string()),
        },
    }
}

fn tail_excerpt(tail: &str) -> Option<String> {
    let t = tail.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.lines().last().unwrap_or(t).to_string())
    }
}

/// Project an entity's optional ref + cached resolution into the honest `Derived`
/// display state for a read. The read-path policy lives here:
/// - **no ref** ⇒ `Asserted` (the v0 hand-set path, surfaced as unverified).
/// - **note ref** ⇒ `Asserted` (manual, never proven).
/// - **file_anchor** ⇒ re-resolve **live** now (always fresh; edits reflected),
///   surfacing the source artifact's age.
/// - **command** ⇒ by default re-resolve **live** now (Manifesto #9: no cache
///   window for a passed verdict to outlive reality, age=0, not served-from-cache);
///   with `cmd_cache` on (opt-in for expensive commands), serve `cached` instead —
///   `None` ⇒ `Unresolved`, and a past-freshness verdict projects to `Stale`.
///
/// `cmd_cache` is supplied by the caller (the cli reads `MUSTER_CMD_CACHE`) so this
/// resolution policy stays deterministic and unit-testable without env state.
pub fn project(
    r: Option<&Ref>,
    cached: Option<&Resolution>,
    now_iso: &str,
    freshness_secs: i64,
    cmd_cache: bool,
) -> Derived {
    let r = match r {
        Some(r) => r,
        None => return Derived::Asserted,
    };
    match r {
        Ref::Note { .. } => Derived::Asserted,
        Ref::FileAnchor { .. } => {
            // Live re-resolution — never stale (the cache is irrelevant here).
            let (res, mtime) = resolve_meta(r, now_iso);
            derived_from(res, now_iso, freshness_secs, false, mtime)
        }
        Ref::Command { .. } => {
            if cmd_cache {
                // Opt-in cache mode: serve the stored verdict; stale past freshness.
                match cached {
                    None => Derived::Unresolved {
                        reason: "command ref not yet resolved — run a resolve to populate it"
                            .to_string(),
                    },
                    Some(res) => derived_from(res.clone(), now_iso, freshness_secs, true, None),
                }
            } else {
                // Honest default: re-resolve live, ignore the (decorative) cache.
                let (res, _) = resolve_meta(r, now_iso);
                derived_from(res, now_iso, freshness_secs, false, None)
            }
        }
    }
}

/// Map a `Resolution` to a `Derived`, applying the staleness rule when the value
/// is served from cache, and computing the freshness evidence (#1): the resolved
/// age (`now − resolved_ts`, saturating ≥ 0; `0` for a live read) and, for a
/// `file_anchor`, the source artifact's mtime age.
fn derived_from(
    res: Resolution,
    now_iso: &str,
    freshness_secs: i64,
    cached: bool,
    source_mtime_epoch: Option<i64>,
) -> Derived {
    match res {
        Resolution::Unresolved { reason } => Derived::Unresolved { reason },
        Resolution::Resolved {
            value,
            outcome,
            resolved_ts,
            source_excerpt,
        } => {
            let now_epoch = store::parse_iso_to_epoch(now_iso).unwrap_or(0);
            let resolved_epoch = store::parse_iso_to_epoch(&resolved_ts).unwrap_or(now_epoch);
            let resolved_age_secs = now_epoch.saturating_sub(resolved_epoch).max(0);
            if cached && is_stale_now(&resolved_ts, now_iso, freshness_secs) {
                Derived::Stale {
                    value,
                    outcome,
                    resolved_ts,
                    resolved_age_secs,
                    served_from_cache: cached,
                }
            } else {
                let source_age_secs =
                    source_mtime_epoch.map(|m| now_epoch.saturating_sub(m).max(0));
                Derived::Derived {
                    value,
                    outcome,
                    resolved_ts,
                    source_excerpt,
                    resolved_age_secs,
                    served_from_cache: cached,
                    source_age_secs,
                }
            }
        }
    }
}

/// A served (cached) resolution is stale when `freshness_secs == 0` (the
/// deterministic hook — never trust a cache) or when its age exceeds the bound.
fn is_stale_now(resolved_ts: &str, now_iso: &str, freshness_secs: i64) -> bool {
    if freshness_secs == 0 {
        return true;
    }
    let resolved = store::parse_iso_to_epoch(resolved_ts).unwrap_or(0);
    let now = store::parse_iso_to_epoch(now_iso).unwrap_or(0);
    domain::is_stale(resolved, now, freshness_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_ref_projects_asserted() {
        assert_eq!(
            project(None, None, "2026-01-01T00:00:00Z", 86_400, false),
            Derived::Asserted
        );
    }

    #[test]
    fn note_ref_projects_asserted() {
        let r = Ref::Note {
            text: "manual".into(),
        };
        assert_eq!(
            project(Some(&r), None, "2026-01-01T00:00:00Z", 86_400, false),
            Derived::Asserted
        );
    }

    #[test]
    fn command_default_mode_resolves_live() {
        // Manifesto #9: with the cache OFF (the honest default), a command ref is
        // re-resolved live on read — no cache needed, age 0, not served-from-cache.
        let r = Ref::Command {
            cmd: "true".into(),
            dir: ".".into(),
        };
        match project(Some(&r), None, "2026-01-01T00:00:00Z", 86_400, false) {
            Derived::Derived {
                resolved_age_secs,
                served_from_cache,
                ..
            } => {
                assert_eq!(resolved_age_secs, 0, "a live read is age 0");
                assert!(!served_from_cache, "a live read is not served from cache");
            }
            other => panic!("expected a live Derived, got {other:?}"),
        }
    }

    #[test]
    fn command_cache_mode_without_cache_is_unresolved() {
        // Re-homed: only in opt-in cache mode does a command ref need a populated
        // cache; absent it, the projection is honestly Unresolved (never green).
        let r = Ref::Command {
            cmd: "just check".into(),
            dir: ".".into(),
        };
        assert!(matches!(
            project(Some(&r), None, "2026-01-01T00:00:00Z", 86_400, true),
            Derived::Unresolved { .. }
        ));
    }

    #[test]
    fn command_cache_goes_stale_at_freshness_zero() {
        // Re-homed to cache-mode (cmd_cache = true): a served verdict past its
        // freshness bound projects to Stale and carries the served age.
        let r = Ref::Command {
            cmd: "just check".into(),
            dir: ".".into(),
        };
        let cached = Resolution::Resolved {
            value: "pass".into(),
            outcome: Outcome::Pass,
            resolved_ts: "2026-01-01T00:00:00Z".into(),
            source_excerpt: None,
        };
        // freshness 0 ⇒ always stale, and it is flagged served-from-cache.
        match project(Some(&r), Some(&cached), "2026-01-01T00:00:10Z", 0, true) {
            Derived::Stale {
                resolved_age_secs,
                served_from_cache,
                ..
            } => {
                assert_eq!(resolved_age_secs, 10);
                assert!(served_from_cache);
            }
            other => panic!("expected Stale, got {other:?}"),
        }
        // generous freshness, same ts ⇒ fresh (still served from cache).
        match project(
            Some(&r),
            Some(&cached),
            "2026-01-01T00:00:00Z",
            86_400,
            true,
        ) {
            Derived::Derived {
                served_from_cache,
                resolved_age_secs,
                ..
            } => {
                assert!(served_from_cache);
                assert_eq!(resolved_age_secs, 0);
            }
            other => panic!("expected Derived, got {other:?}"),
        }
    }
}
