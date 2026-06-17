# Project task runner
# Framework tasks imported from .ckeletin/Justfile

import '.ckeletin/Justfile'

# Single gateway — all checks (CKSPEC-ENF-001)
check: ckeletin-check test ckeletin-health
    @echo "All checks passed."

# Run tests
test:
    cargo nextest run --workspace 2>/dev/null || cargo test --workspace

# Auto-format all code (the write counterpart to `just ckeletin-fmt-check`)
fmt:
    cargo fmt --all

# Run tests with coverage (CKSPEC-TEST-002: 85% minimum).
# Documented exclusion: the conformance generator (.ckeletin/conform) is
# build-time tooling, not shipped runtime code, and is excluded from the
# coverage denominator. Everything else sits at ~99%. CI runs this (see
# the `coverage` job in .github/workflows/ci.yml) so the threshold gates merges.
coverage:
    cargo llvm-cov --workspace --ignore-filename-regex '\.ckeletin/conform/' --fail-under-lines 85

# Build release binary
build:
    cargo build --release

# Check direct dependencies for newer published versions (parity with
# ckeletin-go's `task check:deps:outdated`). Informational — not in `just check`.
outdated: ckeletin-outdated

# Initialize scaffold for a new project (run once after clone).
# `name` is validated by init.sh (lowercase alphanumeric + hyphens). Quoting
# prevents shell word-splitting on any character that passes that validation;
# init.sh rejects names that contain spaces or special characters first.
# Pass `force=true` to bypass the already-initialized guard (use with care).
init name force="false":
    .ckeletin/scripts/init.sh "{{name}}" "{{force}}"

# Smoke-test the scaffold init flow: copy → `just init` → build + test a fresh
# project in a temp dir. Slow (full from-scratch build) so it is #[ignore]d and
# not part of `just check`. Upstream-only — it asserts the `ping` worked example
# survives init, which is not true once a derived project replaces it. CI runs
# this on the ckeletin-rust repo itself (see .github/workflows/ci.yml).
init-smoke:
    cargo test -p ckeletin --test init_smoke -- --ignored
