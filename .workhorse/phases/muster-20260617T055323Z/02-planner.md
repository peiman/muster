# planner (iter 2)

- session: `muster-20260617T055323Z`
- runtime_session_id: `8adfe34d-7d04-45cd-8688-010c38d17951`
- duration_ms: 229635
- cost_usd: $1.492890
- num_turns: 10
- terminal_reason: completed
- stop_reason: end_turn
- is_error: false

---

I've overwritten `PLAN.md` with a re-architected plan that addresses the critic's specific concern.

## What the critic rejected
The old Step 0 assumed a ckeletin scaffold was "already present in this repo root" and only instructed the *rename* — a false premise (`repos/muster/` has only `SPEC.md`, `PLAN.md`, `README.md`, `.workhorse/`; no `crates/`, `Cargo.toml`, `Justfile`).

## What I verified before re-architecting
I read `.ckeletin/scripts/init.sh` directly and confirmed the latent danger behind the critic's CWD warning:
- `init.sh` operates on **relative paths** (acts on CWD, not an argument dir).
- **Step 7 is destructive: `rm -rf .git && git init`** — running it in the wrong directory permanently renames and re-historys that directory, including the shared source framework `/Users/peiman/dev/ckeletin-rust`.
- The guard requires the `peiman/ckeletin-rust` slug; uncommitted changes require `CKELETIN_ASSUME_YES=1` in a non-interactive shell.

## The structural fix
A new **§2.0 Scaffold bootstrap contract** + rewritten **Step 0** make the bootstrap an explicit, fenced sequence:
1. **Hard-assert CWD == `repos/muster`** (`basename "$PWD"` + SPEC/PLAN present) before any init; never `cd` into the source framework.
2. **`rsync -a --exclude='.git/' --exclude='target/' --exclude='.DS_Store'`** copy (no `--delete`, so SPEC/PLAN/.workhorse survive).
3. **`CKELETIN_ASSUME_YES=1 just init muster`** only after re-asserting CWD.
4. New **SC-0 provenance check**: derived tree renamed AND source framework still carries the upstream slug (proves the source was untouched).

The sound remainder the critic endorsed (SC↔DoD mapping, layering, `thiserror` allowlist add — verified `domain = ["serde"]` is current, readiness design, manifesto binding) is retained.

```json
{"rationale": "Re-architects the rejected plan's false 'scaffold already present' premise into an explicit, CWD-asserted bootstrap (§2.0/Step 0): rsync-copy /Users/peiman/dev/ckeletin-rust into repos/muster excluding .git/target while preserving SPEC/PLAN, then run CKELETIN_ASSUME_YES=1 just init muster only after asserting CWD==repos/muster — since init.sh acts on relative paths and does rm -rf .git, this prevents the executor from renaming or re-historying the shared source framework, verified by the new SC-0 provenance check.", "evidence": {"files": ["PLAN.md"]}}
```
