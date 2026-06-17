//! `version` command — the worked example of consuming ckeletin's build-identity
//! primitive. It mirrors the `ping` idiom (a thin CLI handler rendering a
//! `Serialize + Display` type through `Output` in both human and JSON modes); the
//! difference is that `ping` teaches "your own domain type" while this teaches
//! "a framework primitive". The env reads in `current()` are deliberately
//! explicit — not hidden behind a macro — so an adopter can see exactly how the
//! values baked by `build.rs` are wired into [`BuildInfo`].

use infrastructure::build_info::{BuildInfo, UNKNOWN};
use infrastructure::output::Output;
use std::io;

/// Split the baked commit value into `(commit, dirty)`. `build.rs` bakes a bare
/// abbreviated SHA, optionally suffixed `-dirty` — both produced by ONE `git
/// describe` call, so commit and dirty can never disagree (the SH-004 lesson). A
/// hex SHA cannot itself end in `-dirty`, so the suffix is unambiguous.
fn split_commit_dirty(baked: &str) -> (&str, bool) {
    match baked.strip_suffix("-dirty") {
        Some(sha) => (sha, true),
        None => (baked, false),
    }
}

/// The running binary's build identity, from the values `build.rs` baked.
///
/// `option_env!` (not `env!`) so a binary built WITHOUT the build script still
/// compiles and renders an honest "unknown" — never a fabricated commit.
pub fn current() -> BuildInfo {
    let (commit, dirty) =
        split_commit_dirty(option_env!("CKELETIN_BUILD_COMMIT").unwrap_or(UNKNOWN));
    BuildInfo::new(
        env!("CARGO_PKG_VERSION"),
        commit,
        option_env!("CKELETIN_BUILD_DATE").unwrap_or(UNKNOWN),
        dirty,
    )
}

/// Execute the `version` command through the output pipeline.
pub fn execute(output: &Output) -> io::Result<()> {
    output.success("version", &current(), &mut io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_clean_sha() {
        assert_eq!(split_commit_dirty("6e75184"), ("6e75184", false));
    }

    #[test]
    fn split_dirty_sha() {
        assert_eq!(split_commit_dirty("6e75184-dirty"), ("6e75184", true));
    }

    #[test]
    fn split_unknown_is_not_dirty() {
        // An unresolved build degrades to (unknown, not-dirty); the unknown commit
        // is itself the signal, and dirty is meaningless alongside it.
        assert_eq!(split_commit_dirty(UNKNOWN), (UNKNOWN, false));
    }

    #[test]
    fn current_reads_cargo_version() {
        assert_eq!(current().version, env!("CARGO_PKG_VERSION"));
    }
}
