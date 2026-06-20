# Example: governing a spec's feedback cycle

A worked, runnable example of using muster to define and **govern** a real
process — here, how the [ckeletin](https://github.com/peiman/ckeletin) spec
handles feedback from its implementations and updates itself.

It is also the cleanest way to *see muster's whole idea in five minutes*: a
process is a hypothesis, controls point at real sources, and the readiness
verdict can never be greener than reality.

## The process this defines

A consumer (an implementation, an agent, a human) hits friction with the spec
and reports it. The process:

```
intake → triage → DECIDE → implement → verify
                     │
                     ├── UPDATE  the spec (bump version, regenerate)
                     ├── DISCUSS with the reporter (clarify, then re-triage)
                     └── REJECT  with a written reason (never a silent drop)
```

The **decision** is the work an agent or human does. muster does **not** execute
it — muster is the honest scoreboard that reads the real artifacts on every
`readiness` and tells you whether the process is actually being followed:

| Control (glue) | Reads, live | Green only when |
|---|---|---|
| `c-triaged`  | `checks/no_open_feedback.sh` over the register | no report is left untriaged |
| `c-decided`  | `checks/decisions_recorded.sh` over the register | no closed report lacks a decision |
| `c-<consumer>` | each consumer's real `conformance-report.json` → `summary.passed` | that consumer still conforms to the spec |

The teeth: because `c-<consumer>` reads each implementation's **real** conformance
report, a spec change that **breaks a consumer** flips that control red
automatically — the implementations push back on the spec without anyone
remembering to check. (That is the "not master-slave, a learning system" idea,
enforced.)

## Run it

With muster installed (see the repo README → Install) and on your `PATH`:

```sh
cd examples/ckeletin-feedback
./setup-muster.sh        # defines the process + the glue controls in ./.muster
muster readiness         # the live truth-meter

# now play the cycle:
#  1. a report arrives "open"            → readiness shows a gap (c-triaged red)
#  2. triage + record a decision         → edit feedback-register.json
#  3. accept → bump spec + re-conform    → readiness goes READY
#  4. break a consumer's conformance     → that control goes red, READY is gone
```

`setup-muster.sh` is idempotent enough to re-run after `rm -rf .muster`.

## muster is optional — the process degrades gracefully

The **source of truth is the register + the consumer reports**, not muster. The
same enforcement runs with **zero tools** via plain CI:

```sh
./ci-check.sh            # exits non-zero if anything is untriaged / undecided / a consumer broke
```

So a project can adopt this process and enforce it in CI today; muster adds the
living process-map + the agent-drivable readiness view (`muster readiness
--output json`) on top for those who want it. Never make installing muster a
prerequisite for contributing.
