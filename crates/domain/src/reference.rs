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
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Ref {
    /// Read a scalar at a dotted anchor in a TOML or JSON file. The PRIMARY glue.
    /// An optional numeric [`Expectation`] turns the resolved number into an
    /// honest Pass/Fail (e.g. `coverage.percent >= 80`); absent, a bare number
    /// stays `Unknown`. `#[serde(default)]` keeps pre-expectation stores readable.
    FileAnchor {
        path: String,
        anchor: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expect: Option<Expectation>,
    },
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

/// Map a resolved scalar to an outcome. Only an EXPLICIT verdict token derives a
/// Pass/Fail; everything else — a title ("Four-layer architecture") OR a bare
/// number — is `Unknown` (no honesty claim).
///
/// A bare number is a METRIC, not a verdict: muster cannot know whether higher
/// or lower is "good" (`0` errors is green; `0`% coverage is RED), so mapping any
/// number to Pass would FABRICATE a green from an ambiguous signal — the exact
/// "show green when the source is red" lie this tool exists to prevent (#1). So
/// numbers stay `Unknown`; a numeric source becomes a real verdict only with an
/// explicit expectation (a threshold) or a boolean/verdict field. (The legit
/// exit-code-0 = pass case is unambiguous and lives on the SEPARATE command
/// path, `cli::resolve::resolve_command`, which maps `Some(0)` directly.)
pub fn value_to_outcome(value: &str) -> Outcome {
    match value.trim().to_ascii_lowercase().as_str() {
        "met" | "pass" | "passed" | "ok" | "true" | "green" => Outcome::Pass,
        "unmet" | "not_met" | "fail" | "failed" | "false" | "red" => Outcome::Fail,
        _ => Outcome::Unknown,
    }
}

/// A comparator for a numeric [`Expectation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    Ge,
    Le,
    Gt,
    Lt,
    Eq,
}

/// The acceptance criterion that turns a numeric source into an honest verdict.
/// A control points at a metric (e.g. `coverage.percent`) and declares the bar
/// (`>= 80`); muster resolves the number on read and derives Pass/Fail by
/// applying the comparator — NOT by guessing whether higher or lower is "good"
/// (the bare-number honesty hole). The criterion is explicit, so no green is
/// fabricated (#1). `f64` ⇒ `PartialEq` only (no `Eq`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expectation {
    pub op: Comparator,
    pub threshold: f64,
}

impl Expectation {
    /// Whether a resolved number satisfies the criterion.
    ///
    /// The `Eq` arm is an EXACT `f64` comparison by design: `==` is the user's
    /// explicit, literal choice (e.g. `failed_checks == 0`, `open_incidents ==
    /// 0` — integer-valued counts that are exact in `f64`). muster honors what
    /// was written; a tolerance would silently widen the bar. For a measured
    /// float a user picks a range (`>= 79.5`) instead. Hence the `float_cmp`
    /// allow — it is correct here, not an oversight.
    #[allow(clippy::float_cmp)]
    pub fn satisfied_by(&self, n: f64) -> bool {
        match self.op {
            Comparator::Ge => n >= self.threshold,
            Comparator::Le => n <= self.threshold,
            Comparator::Gt => n > self.threshold,
            Comparator::Lt => n < self.threshold,
            Comparator::Eq => n == self.threshold,
        }
    }

    /// Parse an expectation string: `>= 80`, `<=5`, `> 0`, `<10`, `== 100`
    /// (`=` is accepted as `==`). Whitespace-tolerant. A non-finite or
    /// unparseable threshold is rejected (an honest error beats a silent NaN
    /// comparison that would always fail).
    pub fn parse(s: &str) -> Result<Expectation, String> {
        let s = s.trim();
        let (op, rest) = if let Some(r) = s.strip_prefix(">=") {
            (Comparator::Ge, r)
        } else if let Some(r) = s.strip_prefix("<=") {
            (Comparator::Le, r)
        } else if let Some(r) = s.strip_prefix("==") {
            (Comparator::Eq, r)
        } else if let Some(r) = s.strip_prefix('>') {
            (Comparator::Gt, r)
        } else if let Some(r) = s.strip_prefix('<') {
            (Comparator::Lt, r)
        } else if let Some(r) = s.strip_prefix('=') {
            (Comparator::Eq, r)
        } else {
            return Err(format!(
                "expectation must start with one of >= <= > < == (got {s:?})"
            ));
        };
        let threshold: f64 = rest.trim().parse().map_err(|_| {
            format!(
                "expectation threshold must be a number (got {:?})",
                rest.trim()
            )
        })?;
        if !threshold.is_finite() {
            return Err(format!(
                "expectation threshold must be finite (got {threshold})"
            ));
        }
        Ok(Expectation { op, threshold })
    }
}

/// Map a resolved scalar to an outcome, applying an optional numeric
/// [`Expectation`]. With an expectation the value is parsed as a number and the
/// comparator decides Pass/Fail; a non-numeric / non-finite value can't be
/// honestly compared to a numeric bar, so it is `Unknown` (never a fabricated
/// verdict). With no expectation this is exactly [`value_to_outcome`] (explicit
/// verdict tokens pass; bare numbers stay `Unknown`).
pub fn value_to_outcome_with_expect(value: &str, expect: Option<&Expectation>) -> Outcome {
    match expect {
        Some(e) => match value.trim().parse::<f64>() {
            Ok(n) if n.is_finite() => {
                if e.satisfied_by(n) {
                    Outcome::Pass
                } else {
                    Outcome::Fail
                }
            }
            _ => Outcome::Unknown,
        },
        None => value_to_outcome(value),
    }
}

/// The result of dereferencing a Ref — pure data. Built by the cli from the infra
/// resolver's output; consumed by the projection below. Cached on the entity (for
/// `command` refs / display) and re-derived for `file_anchor`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
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
///
/// Manifesto #1 (Truth-Seeking): every *resolved* projection carries the freshness
/// evidence — `resolved_age_secs` (how old the served verdict is, ≥ 0) and
/// `served_from_cache` (whether it came from a stored copy or a live read). A
/// `file_anchor` projection additionally carries `source_age_secs`, the mtime age
/// of the pointed-at artifact, so a confident `met` cannot hide that it derives
/// from a file nobody regenerated.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "resolution_state", rename_all = "snake_case")]
pub enum Derived {
    Derived {
        value: String,
        outcome: Outcome,
        resolved_ts: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source_excerpt: Option<String>,
        /// Age (s) of the served verdict: `now − resolved_ts`, saturating ≥ 0.
        /// `0` for a live read; positive only when served from a cache.
        resolved_age_secs: i64,
        /// `true` only when the verdict came from a stored cache (opt-in command
        /// cache); `false` for every live read (#7 — no stored copy is authority).
        served_from_cache: bool,
        /// Age (s) of the pointed-at source artifact by mtime, for `file_anchor`
        /// refs (#1 — truth is only as fresh as what it points at). `None` for
        /// command/note refs that have no single source file.
        #[serde(skip_serializing_if = "Option::is_none")]
        source_age_secs: Option<i64>,
    },
    Stale {
        value: String,
        outcome: Outcome,
        resolved_ts: String,
        resolved_age_secs: i64,
        served_from_cache: bool,
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

    /// The pointed-at source artifact's mtime age (seconds), for a `file_anchor`
    /// projection. `None` for command/note refs (no single source file) and for
    /// non-`Derived` states. The raw axis the source-freshness policy gates on.
    pub fn source_age_secs(&self) -> Option<i64> {
        match self {
            Derived::Derived {
                source_age_secs, ..
            } => *source_age_secs,
            _ => None,
        }
    }

    /// `true` when this projection's source artifact is older than `bound`
    /// seconds. The source-freshness policy is OPT-IN — `None` ⇒ never
    /// stale-by-source (preserves the default "file_anchor re-resolves live,
    /// never stale" behavior). When a bound is set, a `met` resolved live from an
    /// artifact nobody regenerated past the bound is honestly NOT fresh coverage
    /// (#1) — a distinct axis from the cache `Stale` (which is about the served
    /// *verdict*, not the *source*). Strictly-greater, mirroring [`is_stale`].
    pub fn source_is_stale(&self, bound: Option<i64>) -> bool {
        match (self.source_age_secs(), bound) {
            (Some(age), Some(b)) => age > b,
            _ => false,
        }
    }

    /// Green-eligible only when freshly derived **live** with a non-failing
    /// outcome. A cache-served verdict (`served_from_cache == true`) is never
    /// authority (#7): even within its freshness bound it can freeze a `Pass`
    /// that outlives a now-failing source, so it is excluded here — `drift_profile`
    /// already labels it `cached_command`; this makes that label actually gate green.
    pub fn is_green_eligible(&self) -> bool {
        matches!(
            self,
            Derived::Derived {
                outcome: Outcome::Pass,
                served_from_cache: false,
                ..
            }
        )
    }
}

/// The fixed-set ref-kind **drift profile** for a projected control — the
/// honesty risk profile of its weakest link, surfaced in `readiness` (#9: make
/// the weakest links *visible*, not merely documented). A pure mapping from the
/// ref kind × projected `Derived` × command-cache mode, so it is testable + SSOT.
///
/// Values (a closed set the agent surface can rely on):
/// - `live_resolved` — re-resolved live this read; no drift window.
/// - `cached_command` — a command verdict served from the opt-in cache (drift-prone).
/// - `stale` — a served verdict past its freshness bound (not green-eligible).
/// - `unresolved` — the ref could not be followed.
/// - `asserted` — no honesty claim (note/no-ref).
pub fn drift_profile(r: &Ref, derived: &Derived, cmd_cache_on: bool) -> &'static str {
    match derived {
        Derived::Asserted => "asserted",
        Derived::Unresolved { .. } => "unresolved",
        Derived::Stale { .. } => "stale",
        Derived::Derived { .. } => match r {
            Ref::Note { .. } => "asserted",
            Ref::FileAnchor { .. } => "live_resolved",
            Ref::Command { .. } => {
                if cmd_cache_on {
                    "cached_command"
                } else {
                    "live_resolved"
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_to_outcome_maps_status_tokens() {
        // Only EXPLICIT verdict tokens pass — NOT a bare `0` (see
        // `bare_numbers_are_unknown_not_a_fabricated_pass`).
        for v in ["met", "pass", "PASSED", " ok ", "true", "green"] {
            assert_eq!(value_to_outcome(v), Outcome::Pass, "{v} should pass");
        }
        for v in ["unmet", "not_met", "fail", "FAILED", "false", "red"] {
            assert_eq!(value_to_outcome(v), Outcome::Fail, "{v} should fail");
        }
    }

    #[test]
    fn bare_numbers_are_unknown_not_a_fabricated_pass() {
        // THE honesty hole (dogfood 2026-06-19): a control pointing at a
        // `coverage.percent` anchor resolving to `0` must NOT read green. A bare
        // number is a METRIC, not a verdict — muster cannot know whether higher
        // or lower is "good" (0 errors = good; 0% coverage = RED), so mapping any
        // bare number to Pass FABRICATES a green from an ambiguous signal (#1).
        // It is `Unknown` (honest "unverified"), never Pass. The legit
        // exit-code-0 = pass case lives on the SEPARATE command path
        // (`resolve_command`), which maps `Some(0)` directly — never via here.
        for v in ["0", "0.0", " 0 ", "85", "42", "3.14", "-1", "100"] {
            assert_eq!(
                value_to_outcome(v),
                Outcome::Unknown,
                "a bare number ({v:?}) must be Unknown, never a fabricated Pass"
            );
        }
    }

    #[test]
    fn expectation_parses_comparators_and_rejects_garbage() {
        assert_eq!(
            Expectation::parse(">= 80").unwrap(),
            Expectation {
                op: Comparator::Ge,
                threshold: 80.0
            }
        );
        assert_eq!(Expectation::parse("<=5").unwrap().op, Comparator::Le);
        assert_eq!(Expectation::parse("> 0").unwrap().op, Comparator::Gt);
        assert_eq!(Expectation::parse("<10").unwrap().op, Comparator::Lt);
        assert_eq!(Expectation::parse("== 100").unwrap().op, Comparator::Eq);
        assert_eq!(Expectation::parse("= 1").unwrap().op, Comparator::Eq);
        assert!(Expectation::parse("80").is_err());
        assert!(Expectation::parse(">= abc").is_err());
        assert!(Expectation::parse(">= inf").is_err());
        assert!(Expectation::parse("").is_err());
    }

    #[test]
    fn value_to_outcome_with_expect_derives_an_honest_numeric_verdict() {
        let ge80 = Expectation::parse(">=80").unwrap();
        assert_eq!(
            value_to_outcome_with_expect("85", Some(&ge80)),
            Outcome::Pass
        );
        assert_eq!(
            value_to_outcome_with_expect("0", Some(&ge80)),
            Outcome::Fail
        );
        assert_eq!(
            value_to_outcome_with_expect("80", Some(&ge80)),
            Outcome::Pass,
            "boundary is inclusive for >="
        );
        // A non-numeric source can't honestly meet a numeric bar → Unknown.
        assert_eq!(
            value_to_outcome_with_expect("passed", Some(&ge80)),
            Outcome::Unknown
        );
        assert_eq!(
            value_to_outcome_with_expect("NaN", Some(&ge80)),
            Outcome::Unknown
        );
        // No expectation → exactly value_to_outcome (bare number stays Unknown).
        assert_eq!(value_to_outcome_with_expect("85", None), Outcome::Unknown);
        assert_eq!(value_to_outcome_with_expect("pass", None), Outcome::Pass);
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
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: None,
        };
        assert!(fresh_pass.is_green_eligible());
        let stale = Derived::Stale {
            value: "met".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            resolved_age_secs: 99,
            served_from_cache: true,
        };
        assert!(!stale.is_green_eligible());
        assert!(!Derived::Asserted.is_green_eligible());
        assert!(!Derived::Unresolved { reason: "x".into() }.is_green_eligible());
    }

    /// v2 honesty hole: a command-ref served from the opt-in cache (still within
    /// its freshness bound, so it projects to `Derived` not `Stale`) carries a
    /// frozen `Pass` that can outlive a now-failing source. A cache-served verdict
    /// is NEVER authority (#7) — it must not be green-eligible.
    #[test]
    fn cache_served_pass_is_not_green_eligible() {
        let cached_pass = Derived::Derived {
            value: "pass".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            source_excerpt: None,
            resolved_age_secs: 5,
            served_from_cache: true,
            source_age_secs: None,
        };
        assert!(
            !cached_pass.is_green_eligible(),
            "a cache-served Pass must not be green-eligible"
        );
    }

    #[test]
    fn source_is_stale_is_opt_in_and_only_for_file_anchor_age() {
        // `source_age_secs` is the mtime age of a file_anchor's pointed-at
        // artifact. The source-freshness policy is OPT-IN: `None` ⇒ never
        // stale-by-source (today's behavior). With a bound, an artifact older
        // than it is stale-by-source even though the verdict resolved live — a
        // confident `met` must not hide that it derives from a file nobody
        // regenerated (#1).
        let fresh_src = Derived::Derived {
            value: "met".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: Some(10),
        };
        // opt-in: no bound ⇒ never stale-by-source.
        assert!(!fresh_src.source_is_stale(None));
        // within the bound ⇒ fresh; beyond ⇒ stale (strictly greater).
        assert!(!fresh_src.source_is_stale(Some(10)));
        assert!(fresh_src.source_is_stale(Some(9)));
        assert_eq!(fresh_src.source_age_secs(), Some(10));
        // a projection without a source age (command/note/stale) is never
        // stale-by-source — the axis doesn't apply.
        let no_src = Derived::Derived {
            value: "pass".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: None,
        };
        assert!(!no_src.source_is_stale(Some(0)));
        assert_eq!(no_src.source_age_secs(), None);
        assert!(!Derived::Asserted.source_is_stale(Some(0)));
    }

    #[test]
    fn drift_profile_maps_kind_and_state() {
        let file = Ref::FileAnchor {
            path: "x.toml".into(),
            anchor: "a.b".into(),
            expect: None,
        };
        let cmd = Ref::Command {
            cmd: "true".into(),
            dir: ".".into(),
        };
        let note = Ref::Note { text: "m".into() };
        let live = Derived::Derived {
            value: "pass".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            source_excerpt: None,
            resolved_age_secs: 0,
            served_from_cache: false,
            source_age_secs: Some(3),
        };
        let stale = Derived::Stale {
            value: "pass".into(),
            outcome: Outcome::Pass,
            resolved_ts: "t".into(),
            resolved_age_secs: 99,
            served_from_cache: true,
        };
        // file_anchor live → live_resolved regardless of cache mode.
        assert_eq!(drift_profile(&file, &live, false), "live_resolved");
        assert_eq!(drift_profile(&file, &live, true), "live_resolved");
        // command live: live_resolved when cache off, cached_command when on.
        assert_eq!(drift_profile(&cmd, &live, false), "live_resolved");
        assert_eq!(drift_profile(&cmd, &live, true), "cached_command");
        // stale dominates the kind.
        assert_eq!(drift_profile(&cmd, &stale, true), "stale");
        // unresolved + asserted.
        assert_eq!(
            drift_profile(&file, &Derived::Unresolved { reason: "x".into() }, false),
            "unresolved"
        );
        assert_eq!(drift_profile(&note, &Derived::Asserted, false), "asserted");
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
                anchor: "a.b".into(),
                expect: None,
            }
            .is_cached_kind()
        );
    }

    #[test]
    fn ref_serde_is_tagged_by_kind() {
        let r = Ref::FileAnchor {
            path: "x.toml".into(),
            anchor: "a.b".into(),
            expect: None,
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["kind"], "file_anchor");
        assert_eq!(j["path"], "x.toml");
        let back: Ref = serde_json::from_value(j).unwrap();
        assert_eq!(back, r);
    }
}
