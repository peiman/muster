//! `explain` — an intent-first map (no manual required). Static + deterministic
//! so an agent can diff it. Both surfaces tell the same story (#7).

use crate::view::WithNext;
use infrastructure::output::Output;
use serde::Serialize;
use std::fmt;
use std::io;

#[derive(Serialize)]
struct Intent {
    intent: &'static str,
    command: &'static str,
}

#[derive(Serialize)]
struct Explain {
    tool: &'static str,
    summary: &'static str,
    intents: Vec<Intent>,
}

const INTENTS: &[(&str, &str)] = &[
    ("Start working", "muster init"),
    (
        "Stand up a process (a hypothesis)",
        "muster process add <id> --name <name> --owner <who>",
    ),
    (
        "Compose processes into a map",
        "muster process step add <id> --description <d> --process-ref <sub>",
    ),
    ("See the process graph", "muster process show <id> --tree"),
    (
        "Define a requirement to meet",
        "muster control add <id> --title <t> --clause-ref <ref>",
    ),
    (
        "Point a control at a source (resolve title/status on read, don't copy; --title optional, it derives)",
        "muster control add <id> --ref-file <path> --ref-anchor <dotted.anchor>",
    ),
    (
        "Point a control at a result artifact (the zero-drift safe path — one flag, no re-run)",
        "muster control add <id> --ref-report <result.json> <dotted.anchor>",
    ),
    (
        "Re-resolve a reference (refresh the cached resolution)",
        "muster control resolve <id>",
    ),
    (
        "Re-resolve every ref + flag anchors that silently went unresolved (doctor)",
        "muster control resolve --all",
    ),
    (
        "Import requirements as references from a manifest",
        "muster control import <manifest> --format toml|json --prefix requirements",
    ),
    (
        "Link an implementation (N:M — one requirement, many implementations)",
        "muster control add-implementation <id> --impl-id <iid> --ref-file <path> --ref-anchor <a>",
    ),
    (
        "Prove a control",
        "muster control set-status <id> implemented && muster control attach-evidence <id> <kind> <value>",
    ),
    (
        "Govern a process with a control",
        "muster process link-control <id> <control-id>",
    ),
    (
        "Wire the #9 enforcement seam",
        "muster process check add <id> --description <d> --enforcement <compile_time|lint|script|ci|honor>",
    ),
    (
        "Back a check by a source (derive pass/fail, can't be forged)",
        "muster process check add <id> --description <d> --enforcement ci --ref-cmd \"just check\" --ref-dir <path>",
    ),
    (
        "Ingest a conformance result",
        "muster process check <id> <check-id> --pass|--fail",
    ),
    (
        "Run incident command & control",
        "muster incident report <id> --title <t> --process <pid>",
    ),
    (
        "Record a finding",
        "muster nonconformity raise <id> --from-incident <iid> --description <d>",
    ),
    (
        "Close the #10 feedback loop",
        "muster process revise <id> \"<what changed>\" --because <signal-id>",
    ),
    ("See where you stand", "muster readiness"),
];

impl fmt::Display for Explain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} — {}", self.tool, self.summary)?;
        writeln!(f)?;
        for i in &self.intents {
            writeln!(f, "  {}\n    {}", i.intent, i.command)?;
        }
        Ok(())
    }
}

pub fn execute(output: &Output) -> Result<(), Box<dyn std::error::Error>> {
    let explain = Explain {
        tool: "muster",
        summary: "an AI-first ledningssystem — process map, certification-readiness, incident C2",
        intents: INTENTS
            .iter()
            .map(|(intent, command)| Intent { intent, command })
            .collect(),
    };
    let view = WithNext::new(&explain, "muster init");
    output.success("explain", &view, &mut io::stdout())?;
    Ok(())
}
