# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **muster v3 — the declarative whole-store round-trip (`muster state` /
  `muster apply`).** Two new verbs let an agent author and read the *entire*
  store in one shot, replacing the N+1 per-entity command dance.
  - **`muster state`** serializes the whole store (every process, control,
    incident, nonconformity) to ONE declarative document. `--output json` is the
    authoritative shape; human mode mirrors the same fields. Deterministic,
    id-sorted, and structurally **read-only** (it never mutates or re-resolves
    refs).
  - **`muster apply <manifest>`** reconciles the store to that same shape:
    **upsert** every entity by id (no prune in v3), **idempotent** (a second
    apply is byte-identical), and **fail-closed** — a manifest with a dangling
    `file_anchor` anchor or a malformed shape is refused as a WHOLE (ref
    validation precedes the single writer, so the store is never half-written),
    with the error naming the offending entity and the fix. `--dry-run` prints
    the would-be `readiness` verdict WITHOUT mutating anything. Accepts both the
    `state --output json` envelope and a bare document.
  - The defining invariant is a **fixpoint**: `apply(state())` leaves the store
    identical. One schema flows out (`state`) and in (`apply`) — the domain
    `Store` is the single source (a new pure `domain::StoreDocument`); only the
    cli layer touches disk.
- **`muster readiness --require-ready` — a native CI gate.** Exits **3** when the
  (optionally `--process`-scoped) store is not READY, while still rendering the
  full readiness output (human or JSON) so the operator/agent sees *why*. Without
  the flag, `readiness` always exits `0` (additive, zero regression). Documented
  exit-code contract `0`/`1`/`2`/`3`: `0` = ready / gate passed (or no gate),
  `1` = command error, `2` = CLI usage error (the argument parser's code,
  reserved — never emitted by the gate), `3` = gate not met. Enforces muster's
  "never show green when the source is red" thesis at the CI exit boundary.

### Changed

- **Honesty: note-only evidence no longer counts toward READY.** A hand-set
  control marked `implemented` now needs at least one *verifying* artifact
  (`file`/`url`); a `note` alone is honor-level and surfaces a
  `control_honor_evidence` gap. Symmetric with how a note *ref* already projects
  to `asserted`. (Breaking for stores that relied on note-only coverage.)
- **Honesty: a verifying artifact must actually resolve (honor-VERIFIED).** A
  `file` evidence now counts toward coverage only if the path resolves to an
  existing file (cwd-relative at read time, like `--ref-file`; a directory or a
  missing path does not count); a `url` only if it is well-formed
  (`http(s)://host` — a FORMAT check only, NO-NETWORK, never a reachability
  probe). A control whose only evidence is a missing file or a malformed url is a
  coverage gap with a new `control_evidence_unresolved` finding that names the
  offending artifact and the fix command. Default-on — a named-but-absent
  artifact never reads green. (Breaking for stores that pointed `file`/`url`
  evidence at artifacts that are not present at read time.)
- **Honesty: honor-VERIFIED now gates `proven` processes too.** An active
  process is listed `proven` only when at least one of its evidence items is a
  verifying artifact that RESOLVES (an existing file / a well-formed url),
  mirroring control coverage; a process whose only evidence is a missing file or
  a malformed url — like a note-only one — is `asserted`, not `proven`. Closes a
  false-green where a serialized truth claim backed by an absent artifact read
  proven. A `url` host that is empty-after-trim or contains whitespace
  (`http://  `, `https://x /y`) is now correctly rejected as malformed.

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
