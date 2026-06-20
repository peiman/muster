# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Honesty: note-only evidence no longer counts toward READY.** A hand-set
  control marked `implemented` now needs at least one *verifying* artifact
  (`file`/`url`); a `note` alone is honor-level and surfaces a
  `control_honor_evidence` gap. Symmetric with how a note *ref* already projects
  to `asserted`. (Breaking for stores that relied on note-only coverage.)

### Added

- **Opt-in source-freshness gating** via `MUSTER_SOURCE_FRESHNESS_SECS`: a
  `file_anchor` whose source artifact's mtime exceeds the bound is flagged
  `ref_source_stale` and held back from coverage, even though the verdict
  resolved live. Default unset ⇒ unchanged behavior. `source_age_secs` was
  surfaced but inert before this.
- **Cache-mode warning**: `readiness` and `control resolve --all` now carry a
  `cmd_cache_mode` flag and print a warning when `MUSTER_CMD_CACHE` is on, so the
  weakened (cache-served, drift-prone) honesty guarantee is visible.
- A `Configuration (environment)` section in the README documenting every env
  knob and the two core honesty rules.
