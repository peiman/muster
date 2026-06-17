# ckeletin-rust — Project Guide for AI Agents

## About This Project

**ckeletin-rust** is a Rust CLI scaffold implementing the [ckeletin spec](https://github.com/peiman/ckeletin) (spec version tracked in CONFORMANCE.md / `conformance-mapping.toml`, not hardcoded here). It enforces four-layer architecture at compile time through a Cargo workspace with separate crates.

Key characteristics:
- **Workspace with 5 members:** `crates/domain`, `crates/infrastructure`, `crates/cli`, `.ckeletin/crate` (framework runtime), `.ckeletin/conform` (conformance generator)
- **Compile-time architecture enforcement:** Crate boundaries in Cargo.toml prevent reverse dependencies. Violation tests prove it (CKSPEC-ENF-006)
- **Three-stream output:** stdout (data), stderr (status), log file (audit)
- **JSON mode:** `--output json` flag for machine-readable output on every command
- **Shadow logging:** every output logged to an audit file (on by default, at `~/.config/<app>/logs/`; `--no-audit` or the `log_file_enabled` config to change)
- **Layered configuration:** defaults → TOML file → environment variables → CLI flags
- **TDD:** Tests first, always. 85% minimum coverage
- **Dependency injection over mocking** — writer injection pattern, no mock frameworks

## Architecture

```
.ckeletin/              # VENDORED FRAMEWORK — see section below
├── crate/src/          # Framework runtime modules (config, logging, output, …)
├── conform/            # Conformance generator binary
├── scripts/            # init.sh and other framework scripts
├── Justfile            # Framework recipes (ckeletin-*)
└── VERSION             # Current framework version

crates/
├── domain/             # Business logic — serde ONLY, no framework deps
│   └── src/
│       ├── lib.rs
│       └── ping.rs     # Example: pure function, returns typed result
├── infrastructure/     # Re-export shim — imports .ckeletin/crate, exposes it to cli
│   └── src/
│       └── lib.rs      # Re-exports only: `pub use ckeletin::*;`
└── cli/                # Entry + commands — depends on domain + infrastructure
    └── src/
        ├── main.rs     # Bootstrap only: parse, config, logging init, dispatch, error rendering
        ├── root.rs     # clap derive: Cli struct, Commands enum, OutputFormat
        ├── ping.rs     # Handler: calls domain, renders via infrastructure
        ├── version.rs  # Handler: consumes ckeletin::build_info::BuildInfo framework primitive
        └── catalog.rs  # Handler: walks clap command tree → CKSPEC-AGENT-006 catalog
```

**Dependency direction (compile-time enforced):**
- `domain` → serde only. Cannot import clap, figment, tracing, infrastructure.
- `infrastructure` → re-exports `.ckeletin/crate` (`ckeletin`). Cannot import domain or cli.
- `cli` → Imports both domain and infrastructure. Only crate with clap.

**Violation tests:** `crates/domain/tests/architecture_violations.rs` and `crates/infrastructure/tests/architecture_violations.rs` use `trybuild` to verify that violating a boundary produces a compile error.

## .ckeletin/ — Vendored Framework

`.ckeletin/` is the **vendored ckeletin framework**. It is framework-owned and **replaced wholesale** by `just ckeletin-update`. Do not hand-edit files inside `.ckeletin/` — changes are clobbered on update without warning. The `ckeletin-health` recipe warns when `.ckeletin/` has local modifications.

**Framework surface (agent-drivable):**

| Recipe | Description |
|--------|-------------|
| `just ckeletin-health` | Verify framework version, warn on local `.ckeletin/` modifications, exit non-zero on broken workspace |
| `just ckeletin-version` | Print framework version string |
| `just ckeletin-doctor [json]` | Report dev environment: framework version, toolchain, and tool presence. Pass `json` (positional) for machine-readable output: `just ckeletin-doctor json` |
| `just ckeletin-check-update [json]` | Check whether a newer framework version is available. Pass `json` for `{"current","latest","update_available"}`: `just ckeletin-check-update json` |
| `just ckeletin-update [<branch\|tag>]` | Wholesale-replace `.ckeletin/` from upstream. Runs two-tier validation: compile check (tier 1, rolls back on failure), then `just check` (tier 2, leaves tree dirty on failure so you can fix forward). Emits `CKELETIN_UPDATE_RESULT={"status","from","to","committed","rolled_back"}`. Example: `just ckeletin-update v0.2.18` |
| `just ckeletin-update-dry-run [<branch\|tag>]` | Preview what would change without applying. Example: `just ckeletin-update-dry-run v0.2.18` |
| `just ckeletin-update-check-compatibility [<branch\|tag>]` | Apply the update, run `just check`, then restore the previous framework (interrupt-safe trap). No changes kept. Example: `just ckeletin-update-check-compatibility v0.2.18` |
| `just conform [ARGS]` | Run conformance generator against the vendored spec snapshot (hermetic, no network). Upstream-only — no-ops in consumer repos |
| `just conform-refresh` | Fetch the latest spec from upstream and update `conformance/requirements.json`. Upstream-only |
| `just conform-report` | Regenerate `conformance-report.json` from the mapping (CKSPEC-ENF-010). Upstream-only |
| `just init <slug> [true]` | Initialize the scaffold for a new project. The `name` argument is required (positional); pass `true` as the second positional argument to bypass the already-initialized guard: `just init my-app` or `just init my-app true` |

**Tags work for the version argument:** `just ckeletin-update v0.2.18` resolves the tag via `git fetch ckeletin-upstream <tag>` and `FETCH_HEAD`.

**Scaffold-leftover guard:** consumer repos fail `just check` if functional files (`.github/workflows/*.yml`, root `Justfile`, `lefthook.yml`, `Cargo.toml`s, `deny.toml`) still contain the literal `ckeletin-rust` on non-comment, non-gating lines. This guard fires after `just ckeletin-update` so you find leftovers immediately rather than at release time. Exclusions: comment lines, `github.repository ==` gating lines, everything under `.ckeletin/`. On update, the guard may flag your pre-existing `release.yml` — the exact defect that caused ioguard v0.1.0 to publish zero artifacts. See ioguard PR #4 as the worked fix; the current upstream `release.yml` uses `cargo metadata --no-deps` to derive the binary name structurally and is the reference implementation.

## ckeletin-project.toml — Project-Owned Conformance Config

`ckeletin-project.toml` at the workspace root is the **project-owned** file where you declare your layer layout and allowlists.  It lives outside `.ckeletin/` and is **never touched by `just ckeletin-update`**.  This is the SSOT for everything you need to tailor — never edit `.ckeletin/crate/tests/arch_allowlist.rs` or `.ckeletin/crate/tests/violation_drift_guard.rs` (they are framework-owned and clobbered on every update).

### Sections

**`[layers]`** — paths of your layer crates relative to the workspace root.  List form is supported for projects with multiple domain crates:

```toml
[layers]
domain = ["crates/domain"]          # or ["chat-core", "chat-models"]
infrastructure = ["crates/infrastructure"]
cli = ["crates/cli"]
```

**`[allowlists]`** — complete `[dependencies]` sections allowed for each layer.  The scaffold ships with `domain = ["serde"]` and `infrastructure = ["ckeletin"]`.  Add to these lists when your domain legitimately needs more deps — use TOML comments as justifications:

```toml
[allowlists]
# serde: typed serialization for domain result types (CKSPEC-ARCH-004)
# thiserror: typed error definitions (returns data, never panics)
domain = ["serde", "thiserror"]
# ckeletin: framework crate re-export (CKSPEC-ARCH-005)
infrastructure = ["ckeletin"]
```

**`[violation_tests]`** — controls the drift guard.  Set `enabled = false` with a comment if your layout doesn't carry trybuild violation test copies:

```toml
[violation_tests]
# This project enforces boundaries via Cargo.toml alone; no trybuild copies.
enabled = false
```

### Behavior matrix

| Condition | What the conformance tests do |
|-----------|-------------------------------|
| `ckeletin-project.toml` present | Enforce strictly per declared layout and allowlists |
| File absent + `crates/domain` exists | Enforce scaffold strict defaults (`domain=["serde"]`) |
| File absent + no scaffold layout | Skip loudly — print a nudge naming `ckeletin-project.toml` |
| File present but malformed TOML | Hard error — never silenced |
| A layer declared as `[]` (empty list) | Nothing enforced for that layer — by design |

**Declare only the layers your architecture actually has — an empty layer
list is an honest answer.** Example: a consumer whose adapter crates import
the core by design has no ckeletin-style infrastructure layer; declaring
`infrastructure = []` states that truthfully instead of forcing a false
mapping. The layers you do declare stay strictly enforced. (Consumer
feedback, 2026-06-10 — agent-chat and ioguard both ship this shape.)

## Commands

| Scenario | Command |
|----------|---------|
| Run all checks | `just check` |
| Run tests only | `just test` |
| Format code | `just fmt` |
| Check formatting | `just ckeletin-fmt-check` |
| Run clippy | `just ckeletin-clippy` |
| License/advisory check | `just ckeletin-deny` |
| Outdated direct deps (informational) | `just outdated` (alias for `just ckeletin-outdated`; needs `cargo install cargo-outdated --locked`; CI tool pins in ci.yml are NOT covered) |
| Coverage (85% min) | `just coverage` |
| Build release binary | `just build` |
| Framework environment report | `just ckeletin-doctor` |
| Framework environment report (JSON) | `just ckeletin-doctor json` |
| Run single crate tests | `cargo test -p domain` |
| Run specific test | `cargo test -p ckeletin --lib output::tests::envelope_success` |

**`just check` is the single gateway.** It runs fmt, clippy, test, deny, health — the same checks in CI and locally (SSOT). Run it before every commit.

**`catalog` subcommand (CKSPEC-AGENT-006):** `cargo run -p cli -- catalog` emits a machine-readable JSON catalog of all commands and flags, derived directly from the live clap command tree. Because the catalog and the parser share one tree, the catalog cannot drift from the actual command set.

## Adding a New Command

Follow these steps exactly. Every file listed below must be touched — skipping any step produces a compile error.

1. **Domain logic** (`crates/domain/src/mycommand.rs`):
   - Pure function, returns a typed result struct
   - `#[derive(Serialize)]` + `impl Display` on the result
   - Unit tests in the same file
   - No framework imports — only `serde` and `std`
   - Declare the module in `crates/domain/src/lib.rs`: `pub mod mycommand;`

2. **CLI handler** (`crates/cli/src/mycommand.rs`):
   - Calls domain function, passes result to `Output::success()`
   - Takes `&Output` as parameter for format selection
   - For a "no-data-to-report" success path (e.g. "no recorded
     history yet", "no pending actions"), call `Output::message()`
     not `Output::success()` with a `&format!("...")` String. The
     helper produces a stable JSON shape (`data: {"message":
     "..."}`) that downstream consumers can rely on; passing a
     bare String to `success` wraps it as a raw string blob in the
     envelope's `data` slot. See `output.rs` for the contract.

3. **Declare the module in `crates/cli/src/main.rs`:**
   - Add `mod mycommand;` alongside the existing `mod ping;`, `mod version;`, `mod catalog;`

4. **Wire the variant into `crates/cli/src/root.rs` (`Commands` enum):**
   - Add a new variant (e.g. `MyCommand`) with a doc comment that becomes the help text

5. **Wire the dispatch arm in `crates/cli/src/main.rs` (`run_inner`):**
   - In the `match cli.command { … }` block inside `run_inner`, add:
     `root::Commands::MyCommand => mycommand::execute(&output).map_err(|e| Box::new(e) as _),`

6. **Wire the subcommand name in `crates/cli/src/main.rs` (`subcommand_name`):**
   - In the exhaustive `match command { … }` inside `subcommand_name`, add:
     `root::Commands::MyCommand => "my-command",`
   - This match has NO default arm by design — omitting a new variant is a compile error, preventing silent fallbacks in error envelopes.

7. **Integration test** (`crates/cli/tests/cli.rs`):
   - Test human mode and JSON mode output using `parse_json_stdout` (already defined in the file)
   - Use `cmd().arg("my-command")` for human-mode tests and `cmd().args(["--output", "json", "my-command"])` for JSON-mode tests
   - Parse-then-assert for JSON fields (not string matching) — rename-proof and readable on failure
   - Do not use `CKELETIN_`-prefixed env vars in tests: `just init` renames this prefix to the project name, so tests that reference it break in derived projects. Use `--config <file>` or `XDG_CONFIG_HOME` instead (see `audit_cmd` helper in the file)

8. **Commit atomically:** Test + implementation in one commit (CKSPEC-TEST-004)

> **Common Mistake: Discovery logic in infrastructure.**
> The natural instinct is to put system discovery (running external processes, querying
> system state) in infrastructure because it uses infrastructure tools like process
> runners. But if that discovery code returns domain types, it creates an
> infrastructure -> domain dependency, violating CKSPEC-ARCH-005. The correct pattern:
> infrastructure provides generic tools (e.g., `process::run_capture`), and the **CLI
> layer** uses those tools to run commands and construct domain types from the results.
> Infrastructure never imports domain.

> **Domain types without business logic is valid.**
> Sometimes a command's domain layer is just typed data structures with
> `#[derive(Serialize)]` + `impl Display` — no computation, no validation, just
> structured output types. That is fine. The "logic" is orchestration in the CLI layer:
> calling infrastructure tools, building domain types from results, and passing them to
> `Output`. Not every domain module needs algorithms; sometimes its value is giving the
> pipeline a typed contract instead of raw strings.

> **Consuming a framework primitive: the `version` command.**
> `ping` shows a command built on your OWN domain type. `version`
> (`crates/cli/src/version.rs`) shows the other case: a command built on a
> FRAMEWORK primitive — `ckeletin::build_info::BuildInfo`. The build identity is
> baked at compile time by `crates/cli/build.rs` (one atomic `git describe
> --dirty`, degrading to `unknown` on any git failure, with a `cargo:warning=`
> emitted at build time when degrading) and read explicitly via
> `option_env!` in `version::current()` — the `env!` wiring is deliberately not
> hidden behind a macro so you can see it. `--version` is wired to the same
> `BuildInfo::version_line()` formatter in `main::parse_args`. Keep, customize,
> or delete it like `ping`.

## Coding Conventions

- **Domain has zero framework deps.** If you need logging in domain, return data and let the CLI layer log it.
- **All output through `Output` struct.** Never `println!` or `eprintln!` in domain or infrastructure. The output system handles stream routing and shadow logging.
- **Domain types handed to `Output::success` must implement both `Serialize` and `Display`.** `Output::success<T: Serialize + Display>` renders via `Display` in human mode and serializes via `Serialize` in JSON mode. One value, two outputs — presentation lives on the type. Implementing only `Serialize` means the type doesn't compile into a `success()` call; implementing only `Display` means JSON mode silently renders a string blob. See `crates/cli/src/ping.rs` for a worked example.
- **No-data success paths use `Output::message()`, not `Output::success()` with a `&format!("...")` string.** The `message` helper writes a human sentence in text mode and an envelope with `data: {"message": msg}` in JSON mode — a stable, structured shape instead of a raw string blob.
- **Error envelopes must identify the failing subcommand.** Capture the command name from `&cli.command` *before* moving `cli` into `run_inner`, thread it into `Output::error`. Use an exhaustive `match` (not a default arm) so new subcommands are a compile error until they declare their own name — no silent fallback. See `crates/cli/src/main.rs::subcommand_name`.
- **Output mode precedence (SSOT):** explicit CLI flag (`--output text|json`) > config file / env (`json = true` or `CKELETIN_JSON=true`) > default (human). `--output text` overrides a `json=true` config in both directions. This is computed once in `main.rs::resolve_output_mode` — not separately in `run` and `run_inner`.
- **Typed configuration.** Add fields to `Config` struct in `.ckeletin/crate/src/config.rs`. figment deserializes at startup — no runtime type assertions.
- **Invalid log level strings are startup errors.** `logging::init` validates `console_level` and `file_level` against the known set (`trace|debug|info|warn|error|off`) and returns `Err` rather than silently accepting a directive string that could empty the audit stream.
- **Error handling:** `thiserror` for typed errors, `Box<dyn Error>` at application boundary.
- **Conventional commits:** `feat:`, `fix:`, `test:`, `docs:`, `refactor:`, `ci:`, `chore:`. Enforced by lefthook commit-msg hook.

## Audit Log Behavior

The audit log (CKSPEC-OUT-004) writes to `~/.config/<app>/logs/` by default, using a daily rolling appender (`tracing-appender`). Log files are named `<stem>.<YYYY-MM-DD>` (e.g. `app.log.2026-06-09`). The notice on first run names the directory and the filename pattern.

**Permissions (Unix):** the log directory is created with mode 0700; each log file is set to mode 0600. These are per-user audit contents and must not be world-readable.

**The appender is non-blocking by design.** Lines are queued to a background worker thread. Under sustained backpressure the queue can overflow and lines are dropped rather than blocking the CLI process. This is the correct trade-off for a CLI audit trail: user-visible latency beats silent blocking. The `LogGuard` returned by `logging::init` must be held until program exit so the worker flushes before the process terminates.

**The guard must outlive error rendering.** The `RunError::PostConfig` variant carries the `LogGuard` so the outer `run()` holds it through `Output::error`, ensuring the shadow-log event for a failing command reaches the audit file before the worker shuts down.

## Build Identity

`crates/cli/build.rs` bakes `CKELETIN_BUILD_COMMIT` and `CKELETIN_BUILD_DATE` at compile time via `git describe --dirty`. If any git command fails (fresh `git init`, no tags, detached HEAD), the build degrades to `"unknown"` and emits a `cargo:warning=` so the degradation is visible at build time — not silent. A project created with `just init` gets a committed git repository; `build.rs` is re-run after the commit so the first build bakes real identity.

## Testing

- **Unit tests:** `#[cfg(test)] mod tests` in each source file
- **Violation tests:** `trybuild` compile-fail tests in `crates/*/tests/`
- **Integration tests:** `assert_cmd` in `crates/cli/tests/cli.rs`
- **Coverage:** `just coverage` (85% minimum, CKSPEC-TEST-002)
- **No mock frameworks.** Use writer injection (pass `&mut dyn Write`) or simple test doubles.
- **Fuzz target (`fuzz_ping`):** a bolero-based generative test under `crates/domain/tests/fuzz_ping.rs`. This is a pedagogical worked example showing the bolero harness pattern for stable-toolchain generative testing. The production `ping` type is trivial, so this target serves as a template — not a meaningful guard for the shipped type itself.

## Platform Notes

CI runs on Linux (Ubuntu). macOS and Linux are the supported development platforms. Windows code paths exist in the source (e.g. `%USERPROFILE%` home resolution, `%APPDATA%` platform-data dir) but are **untested** — CI is Linux-only. Windows users may encounter issues; patches welcome.

## Known Limitations and Design Decisions

**Non-blocking audit appender is lossy by design.** `tracing-appender::non_blocking` drops lines under sustained backpressure rather than blocking the CLI process. For a CLI audit trail, user-visible latency is a worse trade-off than occasional line drops. This behavior is documented and accepted; it is not a bug.

**clap usage errors bypass the JSON envelope.** When a user invokes the CLI with invalid flags or a missing required argument, clap exits with code 2 and writes a human-readable error directly to stderr — it does not go through `Output::error` and does not produce a JSON envelope. This is intentional: clap's error presentation is part of the ecosystem convention for arg-parse errors; wrapping it in a JSON envelope would require intercepting clap's internal error path. Routed to spec feedback (CKSPEC-OUT-002 edge-case) for requirement calibration.

## Patterns for data-driven plug points

When a CKSPEC-compliant CLI grows to support **multiple backends,
runtimes, or providers**, the common pattern is a set of `const`s —
one per plugin — all matching the same struct shape (binary name,
signal strings, templates, keywords). This is a powerful pattern
but it has two specific failure modes that earn their own discipline.

### Capture-before-declare

Constants representing external systems (e.g. TUI ready signals,
CLI flag names, API response markers) MUST be picked from captured
evidence of the real system — never from docs, memory, or a
related implementation. External reality drifts; pinned constants
picked from intuition drift silently. The symptom is the pipeline
mis-classifying state weeks after the constant landed, with green
tests the whole time because the tests were written against the
same incorrect values.

**Discipline:** for every new plug-point constant:

1. Launch the real external system under your wrapper.
2. Capture its output/state in every distinct mode
   (pre-ready, ready, post-invocation, completion, failure).
3. Pick constant values from strings that appear *only* in the
   state they identify. Avoid substrings of text that appears in
   adjacent states.
4. Pin the captures as literals in regression tests that assert
   the picked constants appear in the right state and not the
   wrong ones. When the external system changes, these tests fail
   loudly — not silently at runtime.
5. Commit cites the capture source (file path or transcript).

This discipline was earned the hard way — three separate incidents of
constants picked from intuition drifting silently against the real system,
each with green tests the whole time, before it was written down.

### Cross-plug-point alias tests

When two plug-point constants share a shape, it's easy for one to
accidentally pick a signal that's a substring of another's.
Example: if plugin A declares `ready_signal = "Ready"` and plugin
B declares `completion_signal = "Not ready for input"`, A's signal
false-matches B's pane content.

**Discipline:** add a zero-cost invariant test that, for every
pair of plug-points (A, B) where A ≠ B, asserts no signal in A is
a substring of any signal in B. The test iterates the plug-point
registry, so adding a new plug-point automatically gets guarded
without per-plugin test code.

The invariant to assert: for every pair of plug-points (A, B) with A ≠ B, no
signal string in A is a substring of any signal string in B — iterated over the
plug-point registry so a new plug-point is guarded automatically.

### When these patterns apply (and when they don't)

These patterns apply when the CLI has multiple pluggable backends
represented as data (constants or config). They don't apply when
the CLI has a single runtime, a single protocol, or pure
business logic. Add them when the second plug-point lands — not
speculatively in a single-plugin CLI.

## Troubleshooting

| Problem | Fix |
|---------|-----|
| `just check` fails on fmt | `just fmt` then retry |
| Clippy pedantic warning | Fix it or add targeted `#[allow]` with justification |
| Violation test fails after adding dependency | You probably added a framework dep to domain — remove it |
| `cargo deny check` fails (advisory) | See "Advisory DB floats by design" below |
| `just conform` fails after a spec bump | `just conform-refresh` to pull the new spec, review the diff, reconcile `conformance-mapping.toml` |
| Integration test can't find binary | `cargo build` first, or run via `cargo test -p cli` |

### Advisory DB floats by design

The Rust **toolchain is pinned** (`rust-toolchain.toml` + CI) for reproducible
builds and stable trybuild snapshots. The RustSec **advisory database that
`cargo deny` consults is deliberately NOT pinned** — it floats so new CVEs
against transitive deps surface immediately. So CI can go red with zero commits
here when a fresh advisory lands (a weekly scheduled job catches this even on an
idle repo). That is the security system working. Remediation order:

1. Read the `RUSTSEC-…` id in the cargo-deny output.
2. `cargo update` (or `cargo update -p <crate>`), re-run `just ckeletin-deny`. A
   stale `Cargo.lock` is the usual cause; a patched release clears it.
3. If still flagged with no fix available, add a time-boxed entry to
   `[advisories] ignore` in `deny.toml` with the id, a reason, and a revisit
   date. Remove it once a fix ships.

Do **not** "fix" this by pinning the advisory DB — that hides future CVEs.
