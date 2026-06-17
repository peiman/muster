# Framework Update Mechanism Implementation Plan

> **STATUS: IMPLEMENTED** — This plan was executed and shipped as `.ckeletin/`
> framework v0.2.x (init/update/health flows working as of the initial
> restructuring commit). This file is a **historical record only**. Do NOT
> execute it again — the scaffold already has `.ckeletin/` and running init
> would reset it. The unchecked boxes below reflect that the plan was
> implemented without tracking completion in this file, not that the work
> is undone. See `docs/specs/2026-04-14-framework-update-mechanism.md` for
> the current design state including which items remain deferred.

> ~~**For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.~~

**Goal:** Restructure ckeletin-rust so framework code lives in `.ckeletin/`, project crates have clean names, and `init`/`update`/`migrate` flows work.

**Architecture:** Framework library (`ckeletin` crate) moves into `.ckeletin/crate/`. Project crates rename from `ckeletin-*` to `domain`/`infrastructure`/`cli`. Infrastructure re-exports framework modules. Update replaces `.ckeletin/` from upstream via git checkout.

**Tech Stack:** Rust, Cargo workspaces, bash scripts, just (task runner), trybuild (violation tests)

**Spec:** `docs/specs/2026-04-14-framework-update-mechanism.md`

**Follow-up (not in this plan):** `migrate.sh` for existing projects (workhorse). Built after this ships and the new structure is proven. Workhorse migration is the first test of the update mechanism.

---

### Task 1: Create `.ckeletin/` directory with framework crate

Extract the framework library from `crates/infrastructure/` into `.ckeletin/crate/`.

**Files:**
- Create: `.ckeletin/VERSION`
- Create: `.ckeletin/CHANGELOG.md`
- Create: `.ckeletin/crate/Cargo.toml`
- Create: `.ckeletin/crate/src/lib.rs`
- Create: `.ckeletin/crate/src/output.rs`
- Create: `.ckeletin/crate/src/config.rs`
- Create: `.ckeletin/crate/src/logging.rs`
- Create: `.ckeletin/crate/src/process.rs`

- [ ] **Step 1: Create directory structure and VERSION file**

```bash
mkdir -p .ckeletin/crate/src
mkdir -p .ckeletin/migrations
mkdir -p .ckeletin/scripts
echo "0.2.0" > .ckeletin/VERSION
```

- [ ] **Step 2: Create framework CHANGELOG**

Write `.ckeletin/CHANGELOG.md`:

```markdown
# ckeletin Framework Changelog

## [0.2.0] - 2026-04-14

### Added
- Extracted framework library into `.ckeletin/crate/`
- Output, config, logging, process modules from infrastructure
- Framework update mechanism (`just ckeletin-update`)
- Init flow (`just init name=<name>`)
```

- [ ] **Step 3: Create `.ckeletin/crate/Cargo.toml`**

```toml
[package]
name = "ckeletin"
version = "0.2.0"
edition = "2021"
license = "MIT OR Apache-2.0"
publish = false

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
figment = { version = "0.10", features = ["toml", "env"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-appender = "0.2"
thiserror = "2"
```

Note: The ckeletin crate pins its own dependency versions instead of using workspace dependencies. This is intentional — `.ckeletin/` is replaced wholesale on update, and the framework must be self-contained. The project's workspace dependencies are for project crates only.

- [ ] **Step 4: Copy infrastructure source files into `.ckeletin/crate/src/`**

```bash
cp crates/infrastructure/src/output.rs .ckeletin/crate/src/output.rs
cp crates/infrastructure/src/config.rs .ckeletin/crate/src/config.rs
cp crates/infrastructure/src/logging.rs .ckeletin/crate/src/logging.rs
cp crates/infrastructure/src/process.rs .ckeletin/crate/src/process.rs
```

- [ ] **Step 5: Create `.ckeletin/crate/src/lib.rs`**

```rust
pub mod config;
pub mod logging;
pub mod output;
pub mod process;
```

- [ ] **Step 6: Verify the framework crate compiles standalone**

Run: `cargo check -p ckeletin`

This will fail because `.ckeletin/crate` is not yet a workspace member. That's fixed in Task 2.

- [ ] **Step 7: Commit**

```bash
git add .ckeletin/
git commit -m "feat: create .ckeletin/ framework crate with extracted infrastructure"
```

---

### Task 2: Rename project crates and update workspace

Rename `ckeletin-domain` → `domain`, `ckeletin-infrastructure` → `infrastructure`, `ckeletin-cli` → `cli`. Move conform into `.ckeletin/`. Update workspace members.

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/domain/Cargo.toml`
- Modify: `crates/infrastructure/Cargo.toml`
- Modify: `crates/cli/Cargo.toml`
- Move: `crates/conform/` → `.ckeletin/conform/`

- [ ] **Step 1: Update workspace `Cargo.toml`**

Change `members` from:
```toml
members = ["crates/*"]
```
to:
```toml
members = ["crates/*", ".ckeletin/crate", ".ckeletin/conform"]
```

Remove `crates/conform` from the `crates/*` glob by moving it first (step 5).

- [ ] **Step 2: Rename domain crate**

In `crates/domain/Cargo.toml`, change:
```toml
name = "ckeletin-domain"
```
to:
```toml
name = "domain"
```

- [ ] **Step 3: Rename infrastructure crate and add ckeletin dependency**

In `crates/infrastructure/Cargo.toml`, change:
```toml
name = "ckeletin-infrastructure"
```
to:
```toml
name = "infrastructure"
```

Add under `[dependencies]`:
```toml
ckeletin = { path = "../../.ckeletin/crate" }
```

- [ ] **Step 4: Rename CLI crate and update dependencies**

In `crates/cli/Cargo.toml`, change:
```toml
name = "ckeletin-cli"
```
to:
```toml
name = "cli"
```

Change dependency references:
```toml
ckeletin-domain = { path = "../domain" }
ckeletin-infrastructure = { path = "../infrastructure" }
```
to:
```toml
domain = { path = "../domain" }
infrastructure = { path = "../infrastructure" }
```

Keep `owo-colors = { workspace = true }` — it's a project dependency for terminal styling.

- [ ] **Step 5: Move conform crate to `.ckeletin/`**

```bash
mv crates/conform .ckeletin/conform
```

In `.ckeletin/conform/Cargo.toml`, verify the name is `ckeletin-conform` (it should already be from the earlier work). Update the workspace reference — remove `version.workspace = true` etc. since `.ckeletin/` crates are self-contained:

```toml
[package]
name = "ckeletin-conform"
version = "0.2.0"
edition = "2021"
license = "MIT OR Apache-2.0"
publish = false

[[bin]]
name = "conform"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
ureq = "3"
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: rename project crates to clean names, move conform to .ckeletin/"
```

---

### Task 3: Rewrite imports throughout the codebase

Update all `use ckeletin_domain` → `use domain` and `use ckeletin_infrastructure` → `use infrastructure` in `.rs` files. Update infrastructure `lib.rs` to re-export from ckeletin. Also update test files that reference old crate names.

Note: this is a restructuring task, not feature work. TDD does not apply — we are moving code, not writing new behavior. The verification step (`cargo check`) serves as the correctness gate.

**Files:**
- Modify: `crates/cli/src/main.rs`
- Modify: `crates/cli/src/ping.rs`
- Modify: `crates/infrastructure/src/lib.rs`
- Modify: `crates/infrastructure/tests/logging_init.rs`

- [ ] **Step 1: Update `crates/cli/src/main.rs`**

Replace:
```rust
use ckeletin_infrastructure::{
    config::Config,
    logging::{self, LogConfig},
    output::{Output, OutputMode},
};
```
with:
```rust
use infrastructure::{
    config::Config,
    logging::{self, LogConfig},
    output::{Output, OutputMode},
};
```

- [ ] **Step 2: Update `crates/cli/src/ping.rs`**

The current file uses `use ckeletin_domain as domain;` to alias the prefixed name. Since the crate is now named `domain`, the alias is unnecessary. Replace the import lines only:

```rust
use ckeletin_domain as domain;
use ckeletin_infrastructure::output::Output;
```
becomes:
```rust
use domain::ping;
use infrastructure::output::Output;
```

And update the function body from `domain::ping::execute()` to `ping::execute()` (since we now import the module directly).

- [ ] **Step 3: Update `crates/infrastructure/src/lib.rs`**

Replace:
```rust
pub mod config;
pub mod logging;
pub mod output;
pub mod process;
```
with:
```rust
// Re-export framework modules — project code imports from infrastructure, not ckeletin
pub use ckeletin::config;
pub use ckeletin::logging;
pub use ckeletin::output;
pub use ckeletin::process;
```

- [ ] **Step 4: Update `crates/infrastructure/tests/logging_init.rs`**

This test file uses `use ckeletin_infrastructure::logging`. Replace with:
```rust
use infrastructure::logging;
```

Without this fix, `cargo check --workspace` will fail because the test references the old crate name.

- [ ] **Step 5: Verify compilation**

Run: `cargo check --workspace`

Expected: success. All imports resolve through the new names.

- [ ] **Step 6: Commit**

```bash
git add crates/cli/src/ crates/infrastructure/src/lib.rs crates/infrastructure/tests/logging_init.rs
git commit -m "refactor: rewrite imports to clean crate names"
```

---

### Task 4: Update violation tests and `.stderr` files

The violation tests reference old crate names (`ckeletin_infrastructure`, `ckeletin_domain`). Update them to the new names (`infrastructure`, `domain`) and regenerate `.stderr` files.

**Files:**
- Modify: `crates/domain/tests/violations/domain_imports_infrastructure.rs`
- Modify: `crates/infrastructure/tests/violations/infra_imports_domain.rs`
- Regenerate: all `.stderr` files

- [ ] **Step 1: Update `domain_imports_infrastructure.rs`**

Replace:
```rust
use ckeletin_infrastructure::output::Output;

fn main() {
    let _ = Output::new(ckeletin_infrastructure::output::OutputMode::Human);
}
```
with:
```rust
use infrastructure::output::Output;

fn main() {
    let _ = Output::new(infrastructure::output::OutputMode::Human);
}
```

- [ ] **Step 2: Update `infra_imports_domain.rs`**

Replace:
```rust
use ckeletin_domain::ping::PingResult;
```
with:
```rust
use domain::ping::PingResult;
```

- [ ] **Step 3: Delete all existing `.stderr` files**

```bash
rm crates/domain/tests/violations/*.stderr
rm crates/infrastructure/tests/violations/*.stderr
```

- [ ] **Step 4: Regenerate `.stderr` files**

Run: `TRYBUILD=overwrite cargo test -p domain -- architecture_violations`
Run: `TRYBUILD=overwrite cargo test -p infrastructure -- architecture_violations`

This runs the violation tests, and since the `.stderr` files are missing, trybuild generates them from the actual compiler output.

- [ ] **Step 5: Verify all violation tests pass**

Run: `cargo test -p domain -- architecture_violations`
Run: `cargo test -p infrastructure -- architecture_violations`

Expected: all 6 tests pass.

- [ ] **Step 6: Create violation test templates in `.ckeletin/tests/violations/`**

These templates are the source of truth for init. They use the clean crate names (`domain`, `infrastructure`) so they work without modification when init copies them to a new project.

```bash
mkdir -p .ckeletin/tests/violations
cp crates/domain/tests/violations/domain_imports_clap.rs .ckeletin/tests/violations/
cp crates/domain/tests/violations/domain_imports_figment.rs .ckeletin/tests/violations/
cp crates/domain/tests/violations/domain_imports_infrastructure.rs .ckeletin/tests/violations/
cp crates/domain/tests/violations/domain_imports_tracing.rs .ckeletin/tests/violations/
cp crates/infrastructure/tests/violations/infra_imports_clap.rs .ckeletin/tests/violations/
cp crates/infrastructure/tests/violations/infra_imports_domain.rs .ckeletin/tests/violations/
```

Note: `.stderr` files are NOT copied to the templates. They are project-specific (contain absolute paths) and must be regenerated per project during init.

- [ ] **Step 7: Commit**

```bash
git add crates/domain/tests/ crates/infrastructure/tests/ .ckeletin/tests/
git commit -m "test: update violation tests for clean crate names, add templates"
```

---

### Task 5: Update integration tests and ping demo

The integration tests reference the old binary name `ckeletin-rust` and old crate name strings. Update them.

**Files:**
- Modify: `crates/cli/tests/cli.rs`
- Modify: `crates/domain/src/ping.rs`
- Modify: `crates/cli/src/root.rs`

- [ ] **Step 1: Update `crates/cli/tests/cli.rs`**

The `cmd()` function references the binary name. The binary is still `ckeletin-rust` (this is the scaffold's default — init changes it per project). The ping message references `ckeletin-rust`. The help text references `ckeletin-rust`. These are all correct for the scaffold — no changes needed to the binary name.

Verify the test file compiles with the new crate names. The tests use `Command::cargo_bin("ckeletin-rust")` which is set by `[[bin]] name` in cli's Cargo.toml — this is unchanged.

Run: `cargo test -p cli --test cli`

Expected: all tests pass.

If any test fails due to import resolution or crate name changes, fix accordingly.

- [ ] **Step 2: Verify `crates/domain/src/ping.rs` still compiles**

The ping module uses `"ckeletin-rust is alive"` as the message. This is the scaffold default. No change needed.

Run: `cargo test -p domain`

Expected: all tests pass.

- [ ] **Step 3: Run the full test suite**

Run: `cargo test --workspace`

Expected: all tests pass.

- [ ] **Step 4: Commit (if any fixes were needed)**

```bash
git add -A
git commit -m "test: verify full suite passes with new crate structure"
```

---

### Task 6: Update Justfile to two-level structure

Create `.ckeletin/Justfile` with framework recipes. Update root `Justfile` to import it.

**Files:**
- Create: `.ckeletin/Justfile`
- Modify: `Justfile`

- [ ] **Step 1: Create `.ckeletin/Justfile`**

```just
# ckeletin framework tasks
# These recipes are framework-owned. Do not edit — replaced on update.

ckeletin_version := `cat .ckeletin/VERSION`

# Show framework version and check for local modifications
ckeletin-health:
    @echo "ckeletin framework v{{ckeletin_version}}"
    @if git diff --quiet .ckeletin/ 2>/dev/null; then \
        echo "Status: clean"; \
    else \
        echo "WARNING: .ckeletin/ has local modifications — next update will overwrite them"; \
        git diff --name-only .ckeletin/; \
    fi
    @cargo check --workspace -q && echo "Workspace: compiles" || echo "Workspace: BROKEN"

# Framework quality checks
ckeletin-fmt-check:
    cargo fmt --all -- --check

ckeletin-clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

ckeletin-deny:
    cargo deny check

ckeletin-check: ckeletin-fmt-check ckeletin-clippy ckeletin-deny

# Check if a newer framework version is available
ckeletin-check-update:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! git remote | grep -q ckeletin-upstream; then
        echo "No upstream remote. Run: git remote add ckeletin-upstream https://github.com/peiman/ckeletin-rust.git"
        exit 0
    fi
    git fetch ckeletin-upstream -q
    LOCAL=$(cat .ckeletin/VERSION)
    REMOTE=$(git show ckeletin-upstream/main:.ckeletin/VERSION 2>/dev/null || echo "unknown")
    if [ "$LOCAL" = "$REMOTE" ]; then
        echo "Framework is up to date (v$LOCAL)"
    else
        echo "Update available: v$LOCAL → v$REMOTE"
        echo "Run: just ckeletin-update"
    fi

# Update framework from upstream
ckeletin-update version="main":
    #!/usr/bin/env bash
    set -euo pipefail
    # Pre-flight: no uncommitted changes in .ckeletin/
    if ! git diff --quiet .ckeletin/ 2>/dev/null; then
        echo "Error: .ckeletin/ has uncommitted changes. Commit or revert first."
        exit 1
    fi
    OLD_VERSION=$(cat .ckeletin/VERSION)
    # Add upstream remote if needed
    if ! git remote | grep -q ckeletin-upstream; then
        git remote add ckeletin-upstream https://github.com/peiman/ckeletin-rust.git
    fi
    git fetch ckeletin-upstream
    # Replace .ckeletin/ from upstream
    git checkout ckeletin-upstream/{{version}} -- .ckeletin/
    NEW_VERSION=$(cat .ckeletin/VERSION)
    # Update lockfile
    cargo generate-lockfile
    # Verify
    if ! cargo check --workspace; then
        echo "Error: workspace does not compile after update. Rolling back."
        git checkout HEAD -- .ckeletin/
        cargo generate-lockfile
        exit 1
    fi
    # Show what changed
    echo ""
    echo "Updated ckeletin framework: v$OLD_VERSION → v$NEW_VERSION"
    echo ""
    git diff HEAD -- .ckeletin/CHANGELOG.md || true
    # Commit
    git add .ckeletin/ Cargo.lock
    git commit -m "chore: update ckeletin framework from v$OLD_VERSION to v$NEW_VERSION"

# Preview update without applying
ckeletin-update-dry-run version="main":
    #!/usr/bin/env bash
    set -euo pipefail
    if ! git remote | grep -q ckeletin-upstream; then
        git remote add ckeletin-upstream https://github.com/peiman/ckeletin-rust.git
    fi
    git fetch ckeletin-upstream
    LOCAL=$(cat .ckeletin/VERSION)
    REMOTE=$(git show ckeletin-upstream/{{version}}:.ckeletin/VERSION 2>/dev/null || echo "unknown")
    echo "Current: v$LOCAL → Available: v$REMOTE"
    echo ""
    echo "Files that would change:"
    git diff HEAD..ckeletin-upstream/{{version}} --stat -- .ckeletin/

# Run conformance generator
conform *ARGS:
    cargo run -p ckeletin-conform -q -- {{ARGS}}
```

- [ ] **Step 2: Update root `Justfile`**

Replace the entire file:

```just
# Project task runner
# Framework tasks imported from .ckeletin/Justfile

import '.ckeletin/Justfile'

binary_name := "ckeletin-rust"

# Single gateway — all checks (CKSPEC-ENF-001)
check: ckeletin-check test ckeletin-health
    @echo "All checks passed."

# Run tests
test:
    cargo nextest run --workspace 2>/dev/null || cargo test --workspace

# Run tests with coverage (CKSPEC-TEST-002: 85% minimum)
coverage:
    cargo llvm-cov --workspace --fail-under-lines 85

# Build release binary
build:
    cargo build --release

# Initialize scaffold for a new project (run once after clone)
init name:
    .ckeletin/scripts/init.sh {{name}}
```

- [ ] **Step 3: Verify Justfile imports work**

Run: `just ckeletin-health`

Expected: shows framework version, clean status, workspace compiles.

Run: `just check`

Expected: full check suite passes (fmt, clippy, deny, tests, health).

- [ ] **Step 4: Commit**

```bash
git add .ckeletin/Justfile Justfile
git commit -m "feat: two-level Justfile — framework tasks in .ckeletin/"
```

---

### Task 7: Write the init script

Create `.ckeletin/scripts/init.sh` that initializes a new project from the scaffold.

**Files:**
- Create: `.ckeletin/scripts/init.sh`

- [ ] **Step 1: Create the init script**

```bash
mkdir -p .ckeletin/scripts
```

Write `.ckeletin/scripts/init.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

NAME="${1:?Usage: just init name=<project-name>}"

# Validate name (lowercase, hyphens, no spaces)
if [[ ! "$NAME" =~ ^[a-z][a-z0-9-]*$ ]]; then
    echo "Error: name must be lowercase alphanumeric with hyphens (e.g., 'my-project')"
    exit 1
fi

# Pre-flight: warn about uncommitted changes
if [ -d .git ] && ! git diff --quiet 2>/dev/null; then
    echo "Warning: uncommitted changes exist. Init resets git history — uncommitted work will be lost."
    read -p "Continue? (y/N) " confirm
    if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
        echo "Aborted."
        exit 0
    fi
fi

echo "Initializing scaffold as: $NAME"

# 1. Set binary name
sed -i '' "s/name = \"ckeletin-rust\"/name = \"$NAME\"/" crates/cli/Cargo.toml
sed -i '' "s/name = \"ckeletin-rust\"/name = \"$NAME\"/" crates/cli/src/root.rs

# 2. Update workspace metadata
sed -i '' "s|peiman/ckeletin-rust|peiman/$NAME|g" Cargo.toml

# 3. Update Justfile binary name
sed -i '' "s/binary_name := \"ckeletin-rust\"/binary_name := \"$NAME\"/" Justfile

# 4. Update ping message to use new name
sed -i '' "s/ckeletin-rust is alive/$NAME is alive/g" crates/domain/src/ping.rs
sed -i '' "s/ckeletin-rust/$NAME/g" crates/cli/tests/cli.rs

# 5. Strip demo code
# Remove ping domain module
rm -f crates/domain/src/ping.rs
sed -i '' '/pub mod ping;/d' crates/domain/src/lib.rs

# Remove ping CLI command
rm -f crates/cli/src/ping.rs
sed -i '' '/mod ping;/d' crates/cli/src/main.rs
# Remove Ping variant from Commands enum and its match arm
sed -i '' '/Ping,/d' crates/cli/src/root.rs
sed -i '' '/Check connectivity/d' crates/cli/src/root.rs
sed -i '' '/Commands::Ping/d' crates/cli/src/main.rs

# Remove ping-related integration tests (lines containing "ping")
sed -i '' '/ping/Id' crates/cli/tests/cli.rs

# 6. Reset CHANGELOG.md
cat > CHANGELOG.md << 'CHANGELOG'
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
CHANGELOG

# 7. Verify
echo "Verifying..."
if cargo check --workspace -q; then
    echo "Workspace compiles."
else
    echo "Error: workspace does not compile after init. Something went wrong."
    exit 1
fi

# 8. Reset git history
CKELETIN_VERSION=$(cat .ckeletin/VERSION)
rm -rf .git
git init
git add -A
git commit -m "Initial scaffold from ckeletin-rust v$CKELETIN_VERSION"
git tag -a "v0.0.0" -m "Initial scaffold"

echo ""
echo "Done! $NAME is ready."
echo "  Binary: cargo run -p cli"
echo "  Tests:  just check"
```

- [ ] **Step 2: Make executable**

```bash
chmod +x .ckeletin/scripts/init.sh
```

- [ ] **Step 3: Commit**

```bash
git add .ckeletin/scripts/init.sh
git commit -m "feat: init script for new project scaffolding"
```

---

### Task 8: Remove framework code duplicates from infrastructure

The infrastructure crate now re-exports from ckeletin. Remove the duplicated source files and their tests. Keep the crate's Cargo.toml deps that are needed for re-exports.

**Files:**
- Delete: `crates/infrastructure/src/output.rs`
- Delete: `crates/infrastructure/src/config.rs`
- Delete: `crates/infrastructure/src/logging.rs`
- Delete: `crates/infrastructure/src/process.rs`
- Modify: `crates/infrastructure/Cargo.toml` — remove direct deps now provided by ckeletin
- Delete: `crates/infrastructure/tests/logging_init.rs` — tests framework code that now lives in ckeletin

- [ ] **Step 1: Delete duplicated source files**

```bash
rm crates/infrastructure/src/output.rs
rm crates/infrastructure/src/config.rs
rm crates/infrastructure/src/logging.rs
rm crates/infrastructure/src/process.rs
```

- [ ] **Step 2: Update `crates/infrastructure/Cargo.toml`**

The infrastructure crate now only needs `ckeletin` as a dependency (for re-exports) plus any project-specific deps. Remove the framework deps that are now provided transitively through ckeletin:

```toml
[package]
name = "infrastructure"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
ckeletin = { path = "../../.ckeletin/crate" }

[dev-dependencies]
tempfile = { workspace = true }
rstest = { workspace = true }
trybuild = { workspace = true }
```

Note: `serde`, `serde_json`, `figment`, `tracing`, `tracing-subscriber`, `tracing-appender`, `thiserror` are all removed — they're now dependencies of the `ckeletin` crate. Project code accesses them through `ckeletin`'s re-exports. If the project needs direct access to any of these (e.g., `serde::Serialize` on domain types), those are declared in the crate that needs them (domain already has `serde`).

- [ ] **Step 3: Move framework tests to `.ckeletin/crate/`**

The `logging_init.rs` integration test tests framework logging behavior. Move it:

```bash
mkdir -p .ckeletin/crate/tests
mv crates/infrastructure/tests/logging_init.rs .ckeletin/crate/tests/logging_init.rs
```

Update any import references in the test if needed (it should use `ckeletin::logging` instead of `infrastructure::logging` — check and update).

- [ ] **Step 4: Verify compilation**

Run: `cargo check --workspace`

Expected: success. Infrastructure re-exports from ckeletin, CLI uses infrastructure.

- [ ] **Step 5: Run full test suite**

Run: `cargo test --workspace`

Expected: all tests pass. Framework tests now run under the ckeletin crate. Violation tests pass. Integration tests pass.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: remove duplicated framework code from infrastructure"
```

---

### Task 9: Run `just check` and fix any remaining issues

Full verification pass.

**Files:** Any that need fixing.

- [ ] **Step 1: Run `just check`**

Run: `just check`

Expected: fmt-check, clippy, deny, tests, and ckeletin-health all pass.

- [ ] **Step 2: Fix any issues found**

If clippy warnings, fmt issues, or test failures appear, fix them. Common issues:
- Unused imports in infrastructure (removed modules)
- Missing re-exports (if cli code used something from infrastructure that wasn't explicitly re-exported)
- `.stderr` files with stale crate names (regenerate with `TRYBUILD=overwrite`)

- [ ] **Step 3: Run `just conform`**

Run: `just conform`

Expected: conformance generator runs, reports status. Some checks may fail if the conformance mapping still references old crate names in check commands. Fix any stale references in `conformance-mapping.toml`:
- Replace `ckeletin-infrastructure` with `infrastructure` in check commands
- Replace `ckeletin-cli` with `cli` in check commands
- Replace `ckeletin-conform` paths if needed

- [ ] **Step 4: Commit fixes**

```bash
git add -A
git commit -m "fix: resolve post-restructure issues"
```

---

### Task 10: Tag the release and push

**Files:** None (git operations only).

- [ ] **Step 1: Update scaffold CHANGELOG.md**

Add an entry in the project CHANGELOG.md under `[Unreleased]`:

```markdown
### Changed
- Restructured to `.ckeletin/` framework model — framework code in `.ckeletin/crate/`, project crates renamed to `domain`/`infrastructure`/`cli`
- Added `just init`, `just ckeletin-update`, `just ckeletin-health` commands
- Infrastructure crate now re-exports from ckeletin framework crate
```

- [ ] **Step 2: Final verification**

Run: `just check`

Expected: all green.

- [ ] **Step 3: Commit and tag**

```bash
git add -A
git commit -m "feat: ckeletin-rust v0.2.0 — framework update mechanism"
git tag -a v0.2.0 -m "Framework update mechanism: .ckeletin/ model with init and update flows"
```

- [ ] **Step 4: Push**

```bash
git push origin main --tags
```
