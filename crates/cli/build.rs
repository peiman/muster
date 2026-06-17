//! Worked example: bake the binary's build identity into compile-time env vars
//! that `crates/cli/src/version.rs` reads. This is the wiring half of ckeletin's
//! build-identity primitive — adopters keep, customize, or delete it like the
//! `ping` command. Best-effort: any git failure degrades to an honest "unknown",
//! never a build failure and never a fabricated value.

use std::process::Command;

fn main() {
    // Rebuild when HEAD or the index moves, so the baked identity stays honest.
    // `--git-path` resolves the real location even through worktree/submodule
    // indirection (where `.git` is a file, not a directory).
    //
    // Fallback for no-git builds (e.g. `just init` runs `cargo check` BEFORE
    // `git init`): when `--git-path` fails, emit a rerun trigger for the
    // conventional workspace-root path. cargo treats a missing
    // `rerun-if-changed` path as always-dirty, so the script reruns on the
    // next build — including the one after `git init` creates the .git dir.
    // Trade-off: non-git tarball installs also trigger a rerun each build, but
    // since the emitted env vars don't change (still "unknown"), cargo does NOT
    // recompile the crate itself — only this cheap script re-executes.
    let mut found_git = false;
    for p in ["HEAD", "index"] {
        if let Some(path) = git(&["rev-parse", "--git-path", p]) {
            println!("cargo:rerun-if-changed={path}");
            found_git = true;
        }
    }
    if !found_git {
        // No .git yet (e.g. scaffold init before `git init`). Point at the
        // expected location relative to this crate (crates/cli → ../../.git).
        // Once `git init` creates the file, the next build reruns this script
        // and bakes the real commit SHA.
        println!("cargo:rerun-if-changed=../../.git/HEAD");
    }
    println!("cargo:rerun-if-changed=build.rs");

    // ONE command resolves the commit SHA and the dirty marker together, so there
    // is no independent-failure gap where a dirty check fails while the commit
    // read succeeds and bakes a false-clean identity (the two-command trap
    // workhorse hit in SH-004). `--match` with an impossible pattern forces a bare
    // abbreviated SHA rather than a tag-relative string: that keeps `commit` a real
    // SHA AND makes the `-dirty` suffix unambiguous (hex can never end in "-dirty",
    // so a tag named "...-dirty" cannot masquerade as a dirty tree). `--dirty`
    // reflects TRACKED modifications only — git's own semantics, matching
    // ckeletin-go; untracked-only files are not "dirty" here. `version.rs` splits
    // the suffix, so commit and dirty can never disagree.
    let commit = git(&[
        "describe",
        "--always",
        "--abbrev=7",
        "--dirty",
        "--match=__ckeletin_no_such_tag__",
    ])
    .unwrap_or_else(|| {
        println!(
            "cargo:warning=ckeletin build identity unavailable: \
             git describe failed; baking commit=unknown"
        );
        "unknown".to_string()
    });
    // Date is informational; its independent failure degrades to an honest
    // "unknown" date, not a false cleanliness claim — so a separate call is safe.
    let date = git(&["show", "-s", "--format=%cs", "HEAD"]).unwrap_or_else(|| {
        println!(
            "cargo:warning=ckeletin build identity unavailable: \
             git show (date) failed; baking date=unknown"
        );
        "unknown".to_string()
    });

    println!("cargo:rustc-env=CKELETIN_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=CKELETIN_BUILD_DATE={date}");
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}
