//! Fuzzing worked example (bolero) — the pattern to copy for your own targets.
//!
//! This is the `ping` worked example's fuzz counterpart. It feeds an arbitrary
//! message into `PingResult` and asserts two invariants hold for ANY input
//! (including weird unicode, control characters, and very long strings):
//!   1. `Display` never panics.
//!   2. serde serialization never panics and the message round-trips intact.
//!
//! Why bolero: it fuzzes on **stable** Rust (`--sanitizer NONE`), so it doesn't
//! break this scaffold's pinned-stable toolchain — unlike cargo-fuzz, which
//! needs nightly. Crucially, the same `check!()` harness ALSO runs under plain
//! `cargo test`: with no corpus it does a bounded, deterministic pass, so this
//! doubles as a regression guard inside `just check`. To fuzz it actively:
//!
//!   cargo bolero list                       # discover targets
//!   just ckeletin-fuzz fuzz_ping_roundtrip  # time-boxed fuzz run on stable
//!
//! Replace `PingResult` with your own type/parser when you add real input
//! handling — derive `bolero::TypeGenerator` (or `arbitrary::Arbitrary`) on a
//! struct for structure-aware fuzzing.

use domain::ping::PingResult;

#[test]
fn fuzz_ping_roundtrip() {
    bolero::check!().with_type::<String>().for_each(|message| {
        let result = PingResult {
            message: message.clone(),
        };

        // 1. Display must never panic on arbitrary input.
        let _ = format!("{result}");

        // 2. Serialization must not panic, and the message must round-trip.
        let json = serde_json::to_string(&result).expect("PingResult must serialize");
        let back: serde_json::Value =
            serde_json::from_str(&json).expect("serialized PingResult must parse");
        assert_eq!(back["message"].as_str(), Some(message.as_str()));
    });
}
