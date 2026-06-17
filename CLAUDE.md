# Claude-Specific Guide for ckeletin-rust

> Read [AGENTS.md](AGENTS.md) first — it contains all project knowledge.
> This file adds Claude-specific behavioral tuning only.

## Before You Start

Run `just check` to verify the project is green. If it's not, fix it before doing anything else.

## Workflow

1. **TDD always.** Write failing test → verify it fails → implement → verify it passes → commit. No exceptions.
2. **Atomic commits.** Test + implementation in one commit. Every commit must pass `just check`.
3. **Conventional commits.** `feat:`, `fix:`, `test:`, `docs:`, etc. Lefthook enforces this.

## Architecture Rules (Compiler-Enforced)

- **Never add framework deps to `crates/domain/Cargo.toml`.** No clap, figment, tracing. Only serde. The compiler will catch violations, but don't create them in the first place.
- **Never add domain or cli deps to `crates/infrastructure/Cargo.toml`.** Infrastructure provides services, it doesn't consume business logic.
- **Never write to stdout/stderr from domain code.** Return data, let the output system render it.

## When Adding Code

- Follow the pattern in `crates/domain/src/ping.rs` for domain logic
- Follow the pattern in `crates/cli/src/ping.rs` for command handlers
- Follow the pattern in `crates/cli/tests/cli.rs` for integration tests
- Add both human and JSON mode tests for every new command

## Before Declaring Done

Run `just check`. If it passes, the code is correct. If it doesn't, fix it.
