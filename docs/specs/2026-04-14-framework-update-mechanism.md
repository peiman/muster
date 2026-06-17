# Framework Update Mechanism for ckeletin-rust

**Date:** 2026-04-14
**Status:** Shipped (framework v0.2.x) — update/init/health flows implemented; see §Migration Flow for deferred items
**Scope:** Restructure ckeletin-rust to support framework updates, project initialization, and migration of existing projects

## Problem

Projects built from ckeletin-rust receive no upstream improvements. When the scaffold's infrastructure code improves (output, config, logging, conformance tooling), every project must manually copy changes and fix crate name mismatches. This doesn't survive time pressure (Principle 9: Automated Enforcement).

The manual `sed` rename during project creation also breaks `.stderr` violation test files. There is no init flow and no update flow.

## Alternatives Considered

**Git subtree:** Rejected. Subtree merges don't support directory-scoped replacement — conflicts in `.ckeletin/` would leak into manual merge resolution, breaking the wholesale-replace guarantee.

**Cargo patch/workspace inheritance:** Rejected. Workspace inheritance (`dep.workspace = true`) only handles dependency versions, not code delivery. `[patch]` overrides specific crate versions but doesn't provide a mechanism for delivering non-code files (Justfile, scripts, deny.toml).

**Published crate on crates.io:** Deferred, not rejected. The right eventual destination, but premature now — the API isn't stable. Publishing adds ceremony (releases, backwards compatibility) before we know what the stable API is. The vendored model lets us iterate; crates.io locks us in. See "Future: Publishing to crates.io" section.

## Design Principles Applied

- **Truth-Seeking (1):** Framework vs project code has an explicit, enforceable boundary
- **Curiosity Over Certainty (2):** Alternatives explored and documented above. The vendored model is a hypothesis — if it proves wrong, crates.io publishing is the fallback
- **Good Will (3):** The update mechanism is a trust anchor. Every project trusts that `.ckeletin/` delivers safe code because: (a) it comes from a known git remote, (b) `cargo check` verifies every update before commit, (c) rollback is automatic on failure. The framework earns trust through verification, not authority
- **Lean Iteration (4):** Vendored path dependency now; crates.io publishing when API stabilizes
- **Platforms, Not Features (5):** The update mechanism is a platform — every future framework improvement flows through it
- **Partnership (6):** See "Contributing Back to Framework" section
- **Single Source of Truth (7):** Framework code lives in one place (`.ckeletin/`), not duplicated across projects
- **Separation of Concerns (8):** Framework concerns (output, config, logging) separated from project concerns (business logic, commands)
- **Automated Enforcement (9):** Cargo workspace structure enforces architecture boundaries at compile time. `just ckeletin-health` detects local modifications to framework-owned files
- **Feedback Cycle (10):** Projects discover improvements, contribute back via PR, framework distributes to all projects via update. `just ckeletin-check-update` detects when the framework has new versions available

## Key Insight: Rust Doesn't Need Import Rewriting

Go's ckeletin-go requires AST-based import rewriting on every init and update because Go imports are file paths (`github.com/peiman/ckeletin-go/pkg/output`). Forking a project means every import carries the old module name.

Rust separates naming from location. A crate's name is declared in `Cargo.toml` and resolved by Cargo. `use ckeletin::output::Output` works in every project regardless of the project name. The framework crate is always called `ckeletin`. No renaming. No AST tools.

**Clarification on scope of "no rewriting":** This applies to the framework crate (`ckeletin`) and to ongoing updates. The one-time restructuring of the scaffold itself DOES rename the project crates from `ckeletin-domain` → `domain` etc. And migrating existing projects (workhorse) requires a one-time import rewrite. But these are one-time costs during restructure/migration, not recurring costs on every update — which is the key difference from Go's approach.

## Directory Layout

```
project-root/
├── .ckeletin/                          # FRAMEWORK-OWNED — replaced wholesale on update
│   ├── VERSION                         # Framework version (e.g., "0.2.0")
│   ├── CHANGELOG.md                    # Framework change history
│   ├── crate/                          # Framework library crate
│   │   ├── Cargo.toml                  # name = "ckeletin"
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── output.rs               # Envelope, renderer, modes
│   │       ├── config.rs               # Figment loader
│   │       ├── logging.rs              # Tracing setup, guards
│   │       └── process.rs              # Command execution
│   ├── conform/                        # Conformance generator
│   │   ├── Cargo.toml                  # name = "ckeletin-conform"
│   │   └── src/main.rs
│   ├── tests/                          # Violation test templates
│   │   └── violations/
│   ├── migrations/                     # Version-keyed migration scripts
│   │   ├── 0.2.0.sh
│   │   └── ...
│   ├── Justfile                        # Framework recipes (ckeletin-* prefix)
│   └── scripts/
│       ├── init.sh                     # Project initialization
│       ├── update.sh                   # Framework update
│       └── migrate.sh                  # One-time migration for existing projects
│
├── crates/                             # PROJECT-OWNED — never touched by update
│   ├── domain/                         # name = "domain"
│   │   ├── Cargo.toml                  # depends on: serde only
│   │   ├── src/                        # Business logic
│   │   └── tests/violations/           # Architecture violations (from template)
│   ├── infrastructure/                 # name = "infrastructure"
│   │   ├── Cargo.toml                  # depends on: ckeletin + project-specific deps
│   │   └── src/
│   │       ├── lib.rs                  # Re-exports ckeletin + project modules
│   │       └── ...                     # Project-specific modules
│   └── cli/                            # name = "cli"
│       ├── Cargo.toml                  # [[bin]] name = "myproject"
│       ├── src/
│       └── tests/
│
├── Cargo.toml                          # workspace members: ["crates/*", ".ckeletin/crate", ".ckeletin/conform"]
├── Justfile                            # Project recipes, imports .ckeletin/Justfile
├── conformance-mapping.toml            # Project-owned
├── deny.toml                           # Project-owned (framework provides default on init)
├── AGENTS.md
├── CLAUDE.md
├── CHANGELOG.md
└── lefthook.yml
```

**`.ckeletin/` MUST be committed to git** — the update and rollback flows depend on `git checkout HEAD -- .ckeletin/` to restore from the last committed state. Do not add `.ckeletin/` to `.gitignore`.

## Ownership Rules

| Location | Owner | Init | Update | Migrate |
|----------|-------|------|--------|---------|
| `.ckeletin/` | Framework | Untouched | Replaced wholesale | Created from upstream |
| `crates/` | Project | Demo stripped, names set | Never touched | Crates renamed, imports rewritten |
| Root config | Project | Templates created | Never touched | Untouched |
| `Cargo.toml` (workspace) | Project | Members set | Never touched | Members updated |
| `conformance-mapping.toml` | Project | Reset to deferred | Never touched | Untouched |

## Protecting the Framework Boundary

Nothing prevents a developer from editing files inside `.ckeletin/`. The next update silently destroys their changes. To make this visible:

- `just ckeletin-health` checks `git diff --name-only .ckeletin/` and warns if files are locally modified
- `just ckeletin-update` refuses to run if `.ckeletin/` has uncommitted local changes — forces the developer to either commit (accepting they'll be overwritten) or revert
- `just check` includes `ckeletin-health` so CI pipelines catch local modifications

If a project needs to patch framework code: fork ckeletin-rust, make the change in `.ckeletin/`, PR it upstream. After it merges, `just ckeletin-update` delivers it. Local patching is explicitly not supported — it would be silently destroyed.

## Crate Naming

Framework crates carry the `ckeletin` prefix. Project crates are clean:

| Crate | Name | Role |
|-------|------|------|
| `.ckeletin/crate` | `ckeletin` | Framework library — output, config, logging, process |
| `.ckeletin/conform` | `ckeletin-conform` | Conformance generator tool |
| `crates/domain` | `domain` | Business logic — no I/O, no framework deps |
| `crates/infrastructure` | `infrastructure` | Re-exports ckeletin + project-specific I/O |
| `crates/cli` | `cli` | Commands, arg parsing. `[[bin]] name` = project name |

**Current state requires restructuring:** The scaffold currently has `ckeletin-domain`, `ckeletin-infrastructure`, `ckeletin-cli`. The first implementation step renames these to `domain`, `infrastructure`, `cli` and extracts framework code into `.ckeletin/crate/`. This is a one-time restructuring of the scaffold itself.

Project code never writes `use ckeletin::` directly. Infrastructure re-exports framework modules:

```rust
// crates/infrastructure/src/lib.rs
pub use ckeletin::config;
pub use ckeletin::logging;
pub use ckeletin::output;
pub use ckeletin::process;

// Project-specific modules below
```

CLI and domain import from `infrastructure` and `domain`:

```rust
use domain::ping;
use infrastructure::output::Output;
```

The `ckeletin` crate pins its own dependency versions instead of using `[workspace.dependencies]`. This is intentional — `.ckeletin/` is replaced wholesale on update and must be self-contained. The project's workspace dependencies are for project crates only.

## Dependency Graph

```
domain           → serde                            (pure business logic)
infrastructure   → ckeletin, serde, figment...      (framework + project I/O)
cli              → domain, infrastructure, clap     (convergence)
ckeletin         → serde, serde_json, figment,      (framework deps, self-contained)
                   tracing, tracing-subscriber,
                   tracing-appender, thiserror
ckeletin-conform → serde, serde_json, toml, ureq    (standalone tool)
```

Compile-time enforcement: domain has no dependency on infrastructure or ckeletin. Any reverse import is a compile error. Violation tests (trybuild) verify this.

## Init Flow

`just init name=myproject` — run once after cloning.

1. **Pre-flight:** check no uncommitted changes exist (warn and abort if found — prevents losing work when git history is reset in step 8)
2. Set `[[bin]] name` in `crates/cli/Cargo.toml` to `myproject`
3. Update workspace `Cargo.toml` metadata (repository, description)
4. Update root `Justfile` `binary_name` variable
5. Strip demo code:
   - Remove `crates/domain/src/ping.rs` and its `pub mod ping;` in `lib.rs`
   - Remove `crates/cli/src/ping.rs` and its `mod ping;` in `main.rs`
   - Remove `Commands::Ping` variant from `crates/cli/src/root.rs`
   - Remove `root::Commands::Ping => ping::execute(&output)?` match arm from `main.rs`
   - Remove ping-related integration tests from `crates/cli/tests/cli.rs`
6. Copy violation test templates from `.ckeletin/tests/violations/` to `crates/domain/tests/` and `crates/infrastructure/tests/` (templates use `domain` and `infrastructure` as crate names — already correct since project crates have clean names)
7. Reset `CHANGELOG.md` to empty Keep a Changelog template
8. Reset `conformance-mapping.toml` — framework checks `met`, project-specific `deferred`
9. Reset git history: `git init`, initial commit referencing framework version from `.ckeletin/VERSION`, `v0.0.0` tag
10. Verify: `cargo check --workspace`

No crate renaming needed — the scaffold already uses clean names (`domain`, `infrastructure`, `cli`) after the initial restructuring.

## Update Flow

`just ckeletin-update version="main"` — run when framework has improvements.

1. **Pre-flight:** verify no uncommitted changes in `.ckeletin/`: `git diff --quiet .ckeletin/ || abort "uncommitted changes in .ckeletin/ — commit or revert first"`
2. **Save old version:** `OLD_VERSION=$(cat .ckeletin/VERSION)`
3. **Add upstream remote** (first time): `git remote add ckeletin-upstream https://github.com/peiman/ckeletin-rust.git`
4. **Fetch:** `git fetch ckeletin-upstream`
5. **Replace:** `git checkout ckeletin-upstream/$version -- .ckeletin/` (where `$version` is the argument — a branch name or tag like `v0.2.0`)
6. **New version:** `NEW_VERSION=$(cat .ckeletin/VERSION)`
7. **Post-update migrations:** if `OLD_VERSION != NEW_VERSION`, run all migration scripts in `.ckeletin/migrations/` for versions between OLD and NEW in order (e.g., updating from 0.1.0 to 0.3.0 runs `0.2.0.sh` then `0.3.0.sh`). Each migration script is idempotent.
8. **Update lockfile:** `cargo generate-lockfile` to resolve any dependency changes
9. **Verify:** `cargo check --workspace` — if this fails, rollback: `git checkout HEAD -- .ckeletin/` (restores from last committed state), print the error, abort
10. **Show changes:** diff `.ckeletin/CHANGELOG.md` using `git diff HEAD -- .ckeletin/CHANGELOG.md`
11. **Commit:** `git add .ckeletin/ Cargo.lock && git commit -m "chore: update ckeletin framework from $OLD_VERSION to $NEW_VERSION"`

**Dry run:** `just ckeletin-update-dry-run version="main"` — fetches, shows what files would change (`git diff HEAD -- .ckeletin/` after checkout), then rolls back without committing.

**Check for updates:** `just ckeletin-check-update` — fetches upstream, compares `.ckeletin/VERSION` against `ckeletin-upstream/main:.ckeletin/VERSION`, reports whether the project is behind.

No import rewriting. The crate name `ckeletin` is stable across all versions.

## Migration Flow (Existing Projects)

> **Deferred — not implemented; revisit when the first real breaking framework
> change ships (Principle 4 — Lean Iteration).** The design below is the
> intended flow, preserved as the spec for that future implementation. Nothing
> in `.ckeletin/` currently provides `ckeletin-migrate` or `.ckeletin/migrations/`.
> `just ckeletin-update` runs no migration scripts. Build it when the first
> breaking change demands it — not speculatively.

`just ckeletin-migrate prefix=workhorse` — one-time conversion for projects built from the old scaffold.

Example: workhorse has `workhorse-domain`, `workhorse-infrastructure`, `workhorse-cli`.

1. **Dry-run available:** `just ckeletin-migrate-dry-run prefix=workhorse` — shows what would change without applying
2. **Create `.ckeletin/`** from upstream (same as update steps 3-5)
3. **Move conform crate:** `crates/conform/` → `.ckeletin/conform/` (framework-owned after migration)
4. **Rename crates** in all `Cargo.toml` files:
   - `{prefix}-domain` → `domain`
   - `{prefix}-infrastructure` → `infrastructure`
   - `{prefix}-cli` → `cli`
   - Update intra-workspace dependency references to match
5. **Rewrite imports** in all `.rs` files:
   - `{prefix}_domain` → `domain`
   - `{prefix}_infrastructure` → `infrastructure`
6. **Add `ckeletin` dependency** to `crates/infrastructure/Cargo.toml`: `ckeletin = { path = "../../.ckeletin/crate" }`
7. **Add re-exports** to `crates/infrastructure/src/lib.rs`:
   - `pub use ckeletin::{config, logging, output, process};`
8. **Remove framework code** from `crates/infrastructure/src/`: delete modules whose filenames match those in `.ckeletin/crate/src/` (output.rs, config.rs, logging.rs, process.rs). Before deletion, diff each against the scaffold's original — warn if project-specific changes exist that will be lost. Keep all other modules (project-specific, e.g., session.rs).
9. **Update workspace `Cargo.toml`** members: `["crates/*", ".ckeletin/crate", ".ckeletin/conform"]`
10. **Regenerate violation test `.stderr` files:** run `cargo test` with `TRYBUILD=overwrite` to regenerate expected error messages with new crate names
11. **Verify:** `just check` (full suite: fmt, clippy, tests, deny)

After migration, `just ckeletin-update` works going forward. The migration is one-time.

## Contributing Back to Framework

The reverse of the update flow. When a project discovers a framework-level improvement:

1. Fork ckeletin-rust on GitHub
2. Make the change in `.ckeletin/` (the framework code)
3. Open a PR against ckeletin-rust
4. After merge, every project receives the improvement via `just ckeletin-update`

**Local patches to `.ckeletin/` are not supported.** `just ckeletin-update` replaces the entire directory — local modifications are destroyed. `just ckeletin-health` warns about local modifications. This is by design: it forces improvements to flow through the upstream repo where they benefit all projects.

## Justfile Structure

**`.ckeletin/Justfile`** — framework recipes, `ckeletin-` prefixed:

- `ckeletin-update` / `ckeletin-update-dry-run` — framework update
- `ckeletin-check-update` — check if newer framework version exists
- `ckeletin-health` — framework version, compile check, local modification detection
- `ckeletin-check` — fmt, clippy, deny (framework-provided quality gates)
- `conform` — run conformance generator

**Root `Justfile`** — project recipes, imports framework:

```just
import '.ckeletin/Justfile'

binary_name := "myproject"

check: ckeletin-check test
    @echo "All checks passed."

test:
    cargo nextest run --workspace 2>/dev/null || cargo test --workspace

# ... project-specific recipes
```

`deny.toml` stays at project root — project-owned. Framework provides a default during init.

## Version Compatibility and Migrations

> **Deferred — not implemented** (see §Migration Flow above). The convention
> below is the intended design; no migration scripts exist yet and
> `ckeletin-update` does not run them. Revisit when the first breaking change ships.

Framework versions follow semver. Breaking changes between versions are handled by migration scripts.

**Convention:** `.ckeletin/migrations/{version}.sh` — one script per version that introduces breaking changes. Each script is idempotent (safe to run multiple times).

**Version skipping:** The update script reads OLD_VERSION and NEW_VERSION, then runs all migration scripts for versions between them in semver order. Updating from 0.1.0 to 0.3.0 runs `0.2.0.sh` then `0.3.0.sh`.

**No migration needed:** If a version has no breaking changes, there is no migration script for it. The update script skips versions without migration scripts.

## Testing Strategy

**Framework tests:** Unit tests inside `.ckeletin/crate/` run as part of `cargo test --workspace`. These test output, config, logging, process in isolation.

**Violation test templates:** `.ckeletin/tests/violations/` contains the template `.rs` files. During init, these are copied to project `crates/*/tests/violations/`. Since project crates use clean names (`domain`, `infrastructure`), the `.stderr` files in the templates use these names and work without modification. After copy, they are project-owned.

**Update verification:** The shipped two-tier gate:
- Tier 1: `cargo check --workspace` — compile failure triggers rollback via `git restore --source=HEAD --staged --worktree -- .ckeletin/` (wholesale restore, including removal of added files). Emits `CKELETIN_UPDATE_RESULT={"status":"compile_failed",...,"rolled_back":true}`.
- Tier 2: `just check` — if lints/tests fail the update is left in the working tree for manual fix-forward. Emits `CKELETIN_UPDATE_RESULT={"status":"check_failed",...}`.
- Success: committed and emits `CKELETIN_UPDATE_RESULT={"status":"updated",...}`.

**Migration verification:** Deferred — see §Migration Flow.

**Framework health in CI:** `just check` includes `ckeletin-health`. When the workspace does not compile, `ckeletin-health` exits non-zero, causing `just check` to fail. A dirty `.ckeletin/` emits a WARNING (non-fatal) because in CI the checkout is always clean by construction — the warning is for local developer awareness.

## Future: Publishing to crates.io

When the framework API stabilizes, `ckeletin` can be published to crates.io. Projects switch from:
```toml
ckeletin = { path = "../../.ckeletin/crate" }
```
to:
```toml
ckeletin = "1.0"
```

One line change. Updates via `cargo update` with semver protection. The `.ckeletin/` directory would then only contain non-code files (Justfile, scripts, conform, violation templates). The library code moves to crates.io where version bumps signal breaking changes.

This is a future step. The vendored model works now and gives us iteration speed.

## Platform Notes

Init, update, and migration scripts are shell scripts (bash). Cross-platform support (Windows) is a non-goal for the initial implementation. The ckeletin-go scaffold has the same constraint. If Windows support becomes necessary, the scripts can be rewritten in Rust as a `ckeletin` CLI tool.
