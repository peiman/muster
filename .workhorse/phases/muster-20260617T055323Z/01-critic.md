# critic (iter 1)

- session: `muster-20260617T055323Z`
- runtime_session_id: `9dc5ff9a-bd67-4f84-bac2-a16fb8cd318f`
- duration_ms: 113591
- cost_usd: $0.964010
- num_turns: 7
- terminal_reason: completed
- stop_reason: end_turn
- is_error: false

---

I have what I need. Let me render judgment.

## CRITIC assessment — muster v0 plan

**What the plan gets right (substantial):**
- **Solves the user's ask.** Every DoD step (1–9) maps to a mechanically-verifiable SC (SC-1…SC-13): process-as-hypothesis lifecycle, recursive graph + cycle detection, controls/incidents/nonconformities/evidence, the #9 enforcement-ladder check seam, the #10 `--because` revision loop, dual-surface parity, and the readiness truth-meter with the never-green-while-gaps verdict. The manifesto binding (§2.6) is explicit and the validator grades it.
- **Dependencies are clean and vetted.** `serde`, `serde_json`, `thiserror`, `clap`, `assert_cmd`, plus the local ckeletin-rust framework. No network, no database, no async, no unclear-provenance third-party API (the SPEC forbids them; the plan honors that). **No provenance flag warranted** — this is the opposite of the "real-time prices from an unlicensed API" failure mode.
- **Proportionate.** Reuses the existing three-crate split, adds no crate, file-per-entity JSON. Minimalism-as-feature is respected. The one architectural deviation (persistence in `cli`) is reasoned and recorded per #10.

**The blocking defect — a false load-bearing premise in Step 0 (verified, not asserted):**

Step 0.1 begins: *"From a copy of the ckeletin-rust framework **already present in this repo root**, run the rename… `CKELETIN_ASSUME_YES=1 just init muster`."*

I verified this premise with one `ls` (cost: near-zero vs. a full executor+reviewer cycle):
- The repo root (`repos/muster`) contains **only** `PLAN.md`, `SPEC.md`, `README.md`, `.git`, `.workhorse`. There is **no** `crates/`, no `ckeletin-project.toml`, no `Justfile`, no `Cargo.toml`. The copy is **not** present.
- The source framework at `/Users/peiman/dev/ckeletin-rust` exists, and its real scaffold flow is documented in its own `Justfile` as **"copy → `just init` → build"** (the `init-smoke` recipe). The copy must happen *first*; the plan assumes it already happened and never instructs it.

This isn't pedantry, and "the executor will figure it out" is not a substitute (CRITIC mandate). Two concrete hazards:
1. The plan is "authoritative for the Executor" by its own header, yet its first instruction is factually wrong — the executor has nothing to rename.
2. **The dangerous failure mode:** an executor told to run `just init muster` with no scaffold in its CWD may locate the recipe in `/Users/peiman/dev/ckeletin-rust` and run it *there*, renaming the canonical framework — the very framework workhorse itself consumes — into "muster". That corrupts a shared dependency outside the repo. A cheap plan-time fix prevents an expensive, cross-repo cleanup.

**The fix is small and precise** (why reject rather than wave through): prepend an explicit Step 0.0 — copy `/Users/peiman/dev/ckeletin-rust` into the repo root excluding `target/`, `.git/`, and the existing `PLAN.md`/`SPEC.md`/`README.md`/`.workhorse`; then run `just init muster` **with CWD asserted to be `repos/muster`** (guard so the source framework is never mutated); confirm scaffold-scan clean before any feature code.

Everything downstream of a corrected Step 0 is sound. This is the cheap checkpoint working exactly as designed.

```json
{"token": "PLAN_REJECTED", "rationale": "Step 0's load-bearing premise is false and I verified it with one ls: the repo root has no ckeletin scaffold (no crates/, ckeletin-project.toml, Justfile, or Cargo.toml), yet the authoritative plan says to rename a copy 'already present in this repo root' and never instructs the copy. Fix: prepend an explicit copy of /Users/peiman/dev/ckeletin-rust into repos/muster (excluding target/.git and the existing spec docs), then run `just init muster` with CWD asserted to repos/muster so the executor cannot accidentally rename the shared source framework. The rest of the plan — SC↔DoD mapping, vetted no-network deps, proportionate scope, manifesto binding — is sound."}
```
