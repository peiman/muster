//! `reference` — the v1 glue organ: a typed pointer to an authoritative source
//! and the pure projection of a dereferenced value into an honest display state.
//!
//! Manifesto #7 (Single Source of Truth): muster stores a *pointer* (`Ref`),
//! never a copy of the source's title/status — title/status are resolved on read
//! from the pointed-at source. Manifesto #8 (Separation of Concerns): this module
//! is I/O-free — it defines the pure types and the value→outcome / staleness
//! rules only; the fs/process dereference lives in `crates/infrastructure`, and
//! the cli bridges the two. Manifesto #1 (Truth-Seeking): `Unresolved`/`Stale`
//! are surfaced honestly, never silently green.

use serde::{Deserialize, Serialize};

/// A typed pointer to an authoritative source (#7 — reference, don't copy).
/// v1 resolver kinds only (no `url`/network — out of scope).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ref {
    /// Read a scalar at a dotted anchor in a TOML or JSON file. The PRIMARY glue.
    FileAnchor { path: String, anchor: String },
    /// Run a command in a dir; exit 0 = pass, non-zero = fail. Use sparingly.
    Command { cmd: String, dir: String },
    /// Opaque/manual — always surfaced as *asserted*, never proven.
    Note { text: String },
}

impl Ref {
    /// `true` for `command` refs, which serve a cache between explicit resolves
    /// (and therefore go stale past the freshness bound). `file_anchor` refs are
    /// re-resolved live on every read by the cli and never go stale.
    pub fn is_cached_kind(&self) -> bool {
        matches!(self, Ref::Command { .. })
    }
}

/// The honest outcome a resolved value implies. Pure mapping (#1 evidence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
    Unknown,
}

/// Map a resolved scalar to an outcome. A title string ("Four-layer architecture")
/// ⇒ Unknown (title-only, no honesty claim); a status token ⇒ Pass/Fail.
pub fn value_to_outcome(value: &str) -> Outcome {
    match value.trim().to_ascii_lowercase().as_str() {
        "met" | "pass" | "passed" | "ok" | "true" | "green" | "0" => Outcome::Pass,
        "unmet" | "not_met" | "fail" | "failed" | "false" | "red" => Outcome::Fail,
        _ => Outcome::Unknown,
    }
}

/// The result of dereferencing a Ref — pure data. Built by the cli from the infra
/// resolver's output; consumed by the projection below. Cached on the entity (for
/// `command` refs / display) and re-derived for `file_anchor`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Resolution {
    Resolved {
        value: String,
        outcome: Outcome,
        resolved_ts: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_excerpt: Option<String>,
    },
    Unresolved {
        reason: String,
    },
}

/// Pure staleness rule (#1). Epoch seconds in (cli owns the clock, C6); domain
/// just compares. `freshness_secs == 0` ⇒ any cached resolution is immediately
/// stale on the next read (deterministic test hook, SC-7).
pub fn is_stale(resolved_epoch: i64, now_epoch: i64, freshness_secs: i64) -> bool {
    now_epoch.saturating_sub(resolved_epoch) > freshness_secs
}

/// The four honest display states surfaced in JSON (`resolution_state`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "resolution_state", rename_all = "snake_case")]
pub enum Derived {
    Derived {
        value: String,
        outcome: Outcome,
        resolved_ts: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_excerpt: Option<String>,
    },
    Stale {
        value: String,
        outcome: Outcome,
        resolved_ts: String,
    },
    Unresolved {
        reason: String,
    },
    /// No ref → hand-set, surfaced as "asserted (unverified)".
    Asserted,
}

impl Derived {
    /// The outcome this projection implies for honesty checks. `Stale`/
    /// `Unresolved`/`Asserted` never count as a `Pass`.
    pub fn outcome(&self) -> Outcome {
        match self {
            Derived::Derived { outcome, .. } => *outcome,
            // A stale value is shown honestly but cannot prove green.
            Derived::Stale { .. } | Derived::Unresolved { .. } | Derived::Asserted => {
                Outcome::Unknown
            }
        }
    }

    /// Green-eligible only when freshly derived with a non-failing outcome.
    pub fn is_green_eligible(&self) -> bool {
        matches!(
            self,
            Derived::Derived {
                outcome: Outcome::Pass,
                ..
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_to_outcome_maps_status_tokens() {
        for v in ["met", "pass", "PASSED", " ok ", "true", "green", "0"] {
            assert_eq!(value_to_outcome(v), Outcome::Pass, "{v} should pass");
        }
        for v in ["unmet", "not_met", "fail", "FAILED", "false", "red"] {
            assert_eq!(value_to_outcome(v), Outcome::Fail, "{v} should fail");
        }
    }

    #[test]
    fn value_to_outcome_title_and_garbage_are_unknown() {
        assert_eq!(
            value_to_outcome("Four-layer architecture"),
            Outcome::Unknown
        );
        assert_eq!(value_to_outcome("wibble"), Outcome::Unknown);
        assert_eq!(value_to_outcome(""), Outcome::Unknown);
    }

    #[test]
    fn is_stale_boundary() {
        // Exactly at the bound is fresh; one past is stale.
        assert!(!is_stale(0, 10, 10));
        assert!(is_stale(0, 11, 10));
        // freshness 0 ⇒ any age > 0 is stale immediately.
        assert!(is_stale(100, 101, 0));
        assert!(!is_stale(100, 100, 0));
    }

    #[test]
    fn is_stale_is_saturating_on_clock_skew() {
        // now before resolved_ts ⇒ not stale (saturating sub clamps to 0).
        assert!(!is_stale(200, 100, 0));
    }

    #[test]
    fn derived_green_eligibility() {
        let fresh_pass = Derived::Derived {
            value: "met".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            source_excerpt: None,
        };
        assert!(fresh_pass.is_green_eligible());
        let stale = Derived::Stale {
            value: "met".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
        };
        assert!(!stale.is_green_eligible());
        assert!(!Derived::Asserted.is_green_eligible());
        assert!(!Derived::Unresolved { reason: "x".into() }.is_green_eligible());
    }

    #[test]
    fn ref_command_is_cached_kind() {
        assert!(
            Ref::Command {
                cmd: "just check".into(),
                dir: ".".into()
            }
            .is_cached_kind()
        );
        assert!(
            !Ref::FileAnchor {
                path: "x.toml".into(),
                anchor: "a.b".into()
            }
            .is_cached_kind()
        );
    }

    #[test]
    fn ref_serde_is_tagged_by_kind() {
        let r = Ref::FileAnchor {
            path: "x.toml".into(),
            anchor: "a.b".into(),
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["kind"], "file_anchor");
        assert_eq!(j["path"], "x.toml");
        let back: Ref = serde_json::from_value(j).unwrap();
        assert_eq!(back, r);
    }
}
