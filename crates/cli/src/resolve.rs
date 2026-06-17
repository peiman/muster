//! The cli bridge — the only place `domain` and `infrastructure` meet (#8).
//!
//! domain defines the pure `Ref`/`Resolution`/`Derived` types but cannot do I/O;
//! infrastructure does the I/O but cannot see domain. This module owns the clock
//! and the resolution policy: it calls the right infra resolver, maps the infra
//! result to a `domain::Resolution`, and projects it to the honest `Derived`
//! display state — re-resolving `file_anchor` refs live on every read (so edits
//! to the source reflect immediately, SC-3) while serving the cached resolution
//! for `command` refs (which go `Stale` past the freshness bound, SC-7).

use crate::store;
use domain::reference::{Derived, Outcome, Ref, Resolution};
use infrastructure::resolver;

/// Dereference a `Ref` into a `domain::Resolution` via live I/O. `now_iso` is the
/// read clock (the domain stays clock-free, C6).
pub fn resolve(r: &Ref, now_iso: &str) -> Resolution {
    match r {
        Ref::FileAnchor { path, anchor } => {
            let fr = resolver::resolve_file_anchor(path, anchor);
            match fr.value {
                Some(value) => {
                    let outcome = domain::value_to_outcome(&value);
                    Resolution::Resolved {
                        value,
                        outcome,
                        resolved_ts: now_iso.to_string(),
                        source_excerpt: fr.excerpt,
                    }
                }
                None => Resolution::Unresolved {
                    reason: fr.reason.unwrap_or_else(|| "unresolved".to_string()),
                },
            }
        }
        Ref::Command { cmd, dir } => {
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
        // A note is manual — it produces a value but no honesty claim. The
        // projection surfaces it as `Asserted`.
        Ref::Note { text } => Resolution::Resolved {
            value: text.clone(),
            outcome: Outcome::Unknown,
            resolved_ts: now_iso.to_string(),
            source_excerpt: None,
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
/// - **file_anchor** ⇒ re-resolve **live** now (always fresh; edits reflected).
/// - **command** ⇒ serve `cached`; `None` ⇒ `Unresolved`; stale past freshness.
pub fn project(
    r: Option<&Ref>,
    cached: Option<&Resolution>,
    now_iso: &str,
    freshness_secs: i64,
) -> Derived {
    let r = match r {
        Some(r) => r,
        None => return Derived::Asserted,
    };
    match r {
        Ref::Note { .. } => Derived::Asserted,
        Ref::FileAnchor { .. } => {
            // Live re-resolution — never stale (the cache is irrelevant here).
            derived_from(resolve(r, now_iso), now_iso, freshness_secs, false)
        }
        Ref::Command { .. } => match cached {
            None => Derived::Unresolved {
                reason: "command ref not yet resolved — run a resolve to populate it".to_string(),
            },
            Some(res) => derived_from(res.clone(), now_iso, freshness_secs, true),
        },
    }
}

/// Map a `Resolution` to a `Derived`, applying the staleness rule when the value
/// is served from cache.
fn derived_from(res: Resolution, now_iso: &str, freshness_secs: i64, cached: bool) -> Derived {
    match res {
        Resolution::Unresolved { reason } => Derived::Unresolved { reason },
        Resolution::Resolved {
            value,
            outcome,
            resolved_ts,
            source_excerpt,
        } => {
            if cached && is_stale_now(&resolved_ts, now_iso, freshness_secs) {
                Derived::Stale {
                    value,
                    outcome,
                    resolved_ts,
                }
            } else {
                Derived::Derived {
                    value,
                    outcome,
                    resolved_ts,
                    source_excerpt,
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
            project(None, None, "2026-01-01T00:00:00Z", 86_400),
            Derived::Asserted
        );
    }

    #[test]
    fn note_ref_projects_asserted() {
        let r = Ref::Note {
            text: "manual".into(),
        };
        assert_eq!(
            project(Some(&r), None, "2026-01-01T00:00:00Z", 86_400),
            Derived::Asserted
        );
    }

    #[test]
    fn command_without_cache_is_unresolved() {
        let r = Ref::Command {
            cmd: "just check".into(),
            dir: ".".into(),
        };
        assert!(matches!(
            project(Some(&r), None, "2026-01-01T00:00:00Z", 86_400),
            Derived::Unresolved { .. }
        ));
    }

    #[test]
    fn command_cache_goes_stale_at_freshness_zero() {
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
        // freshness 0 ⇒ always stale.
        assert!(matches!(
            project(Some(&r), Some(&cached), "2026-01-01T00:00:00Z", 0),
            Derived::Stale { .. }
        ));
        // generous freshness, same ts ⇒ fresh.
        assert!(matches!(
            project(Some(&r), Some(&cached), "2026-01-01T00:00:00Z", 86_400),
            Derived::Derived { .. }
        ));
    }
}
