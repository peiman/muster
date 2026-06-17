//! `resolver` — the dereference engine (the disk/process boundary, #8).
//!
//! This is the I/O half of the v1 glue: it follows a pointer to an authoritative
//! source and returns what it found. It must NOT depend on `domain` (CKSPEC-ARCH-005
//! — infrastructure cannot reach up), so it returns infra-local plain structs; the
//! cli bridges these to `domain::Resolution`. Read-only: it opens files for reading
//! and runs commands, but never writes the sources it points at (#7 — reference,
//! don't mutate). Total: every failure becomes a `reason`, never a panic (#1).

use std::process::Command;

/// The result of reading a scalar at a file anchor. Infra-local (never crosses
/// the JSON surface — the cli maps it to `domain::Resolution`).
#[derive(Debug, Clone, PartialEq)]
pub struct FileResolution {
    /// The scalar value at the anchor, or `None` when the file/anchor is missing
    /// or the leaf is not a scalar.
    pub value: Option<String>,
    /// Why unresolved (names the missing path/anchor segment). `None` on success.
    pub reason: Option<String>,
    /// A short excerpt of the source for display, when available.
    pub excerpt: Option<String>,
    /// The source file's mtime in epoch seconds, when the file could be read
    /// (#1 — muster's truth is only as fresh as the artifact it points at). The
    /// cli computes the *age* against its clock; infra returns raw mtime only.
    pub source_mtime_epoch: Option<i64>,
}

/// The result of running a command. Infra-local.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandResolution {
    /// The process exit code, or `None` when the spawn itself failed.
    pub exit_code: Option<i32>,
    /// The tail of captured stdout (bounded).
    pub stdout_tail: String,
    /// Why unresolved (spawn failure). `None` when the command ran.
    pub reason: Option<String>,
}

const STDOUT_TAIL_BYTES: usize = 512;

/// Read `path` (TOML if `.toml`, JSON if `.json` — by extension), walk the dotted
/// `anchor`, and return the scalar leaf as a string. All errors → `reason`
/// (never panic). Read-only.
pub fn resolve_file_anchor(path: &str, anchor: &str) -> FileResolution {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return unresolved_file(format!("cannot read file '{path}': {e}"));
        }
    };
    let source_mtime_epoch = source_mtime_epoch(path);
    let value: TomlOrJson = if path.ends_with(".json") {
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => TomlOrJson::Json(v),
            Err(e) => return unresolved_file(format!("cannot parse JSON '{path}': {e}")),
        }
    } else {
        // Default to TOML (covers .toml and extensionless manifests).
        match text.parse::<toml::Value>() {
            Ok(v) => TomlOrJson::Toml(v),
            Err(e) => return unresolved_file(format!("cannot parse TOML '{path}': {e}")),
        }
    };

    match value.walk(anchor) {
        Walked::Scalar(s) => FileResolution {
            excerpt: Some(truncate(&s, 120)),
            value: Some(s),
            reason: None,
            source_mtime_epoch,
        },
        Walked::MissingSegment(seg) => unresolved_file(format!(
            "anchor '{anchor}' not found in '{path}' (missing segment '{seg}')"
        )),
        Walked::NotScalar => unresolved_file(format!(
            "anchor '{anchor}' in '{path}' is not a scalar (expected a string/number/bool leaf)"
        )),
    }
}

/// The mtime of `path` in epoch seconds, or `None` when the metadata/mtime is
/// unavailable (e.g. platform without mtime). Read-only.
fn source_mtime_epoch(path: &str) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let secs = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    i64::try_from(secs).ok()
}

fn unresolved_file(reason: String) -> FileResolution {
    FileResolution {
        value: None,
        reason: Some(reason),
        excerpt: None,
        source_mtime_epoch: None,
    }
}

/// List the immediate child keys of the table at dotted `prefix` in a TOML/JSON
/// file (e.g. the requirement IDs under `requirements`). Read-only; used by
/// `control import` to ingest a manifest as references (#7), not copies. Returns
/// `Err(reason)` on a missing file / unparseable source / missing-or-non-table
/// prefix. Keys are returned id-sorted (deterministic, AX).
pub fn list_keys(path: &str, prefix: &str) -> Result<Vec<String>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read file '{path}': {e}"))?;
    let mut keys: Vec<String> = if path.ends_with(".json") {
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("cannot parse JSON '{path}': {e}"))?;
        let table = walk_to_table_json(&v, prefix)?;
        table.keys().cloned().collect()
    } else {
        let v: toml::Value = text
            .parse()
            .map_err(|e| format!("cannot parse TOML '{path}': {e}"))?;
        let table = walk_to_table_toml(&v, prefix)?;
        table.keys().cloned().collect()
    };
    keys.sort();
    Ok(keys)
}

fn walk_to_table_toml<'a>(
    root: &'a toml::Value,
    prefix: &str,
) -> Result<&'a toml::map::Map<String, toml::Value>, String> {
    let mut cur = root;
    for seg in prefix.split('.').filter(|s| !s.is_empty()) {
        cur = cur
            .get(seg)
            .ok_or_else(|| format!("prefix '{prefix}' not found (missing segment '{seg}')"))?;
    }
    cur.as_table()
        .ok_or_else(|| format!("prefix '{prefix}' is not a table"))
}

fn walk_to_table_json<'a>(
    root: &'a serde_json::Value,
    prefix: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, String> {
    let mut cur = root;
    for seg in prefix.split('.').filter(|s| !s.is_empty()) {
        cur = cur
            .get(seg)
            .ok_or_else(|| format!("prefix '{prefix}' not found (missing segment '{seg}')"))?;
    }
    cur.as_object()
        .ok_or_else(|| format!("prefix '{prefix}' is not an object"))
}

/// Run `cmd` in `dir` via `sh -c`; capture exit code + a bounded stdout tail.
/// Exit 0 = pass, non-zero = fail (the cli maps the code to an outcome); a spawn
/// failure becomes a `reason`. Read-only with respect to muster's own state.
pub fn run_command(cmd: &str, dir: &str) -> CommandResolution {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .output();
    match output {
        Ok(out) => {
            let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
            if s.len() > STDOUT_TAIL_BYTES {
                let start = s.len() - STDOUT_TAIL_BYTES;
                // Snap to a char boundary so the slice is valid UTF-8.
                let start = (start..s.len())
                    .find(|i| s.is_char_boundary(*i))
                    .unwrap_or(s.len());
                s = s[start..].to_string();
            }
            CommandResolution {
                exit_code: out.status.code(),
                stdout_tail: s,
                reason: None,
            }
        }
        Err(e) => CommandResolution {
            exit_code: None,
            stdout_tail: String::new(),
            reason: Some(format!("cannot run '{cmd}' in '{dir}': {e}")),
        },
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let end = (0..=max)
        .rev()
        .find(|i| s.is_char_boundary(*i))
        .unwrap_or(0);
    format!("{}…", &s[..end])
}

enum TomlOrJson {
    Toml(toml::Value),
    Json(serde_json::Value),
}

enum Walked {
    Scalar(String),
    MissingSegment(String),
    NotScalar,
}

impl TomlOrJson {
    fn walk(&self, anchor: &str) -> Walked {
        match self {
            TomlOrJson::Toml(v) => walk_toml(v, anchor),
            TomlOrJson::Json(v) => walk_json(v, anchor),
        }
    }
}

fn walk_toml(root: &toml::Value, anchor: &str) -> Walked {
    let mut cur = root;
    for seg in anchor.split('.') {
        match cur.get(seg) {
            Some(next) => cur = next,
            None => return Walked::MissingSegment(seg.to_string()),
        }
    }
    match cur {
        toml::Value::String(s) => Walked::Scalar(s.clone()),
        toml::Value::Integer(i) => Walked::Scalar(i.to_string()),
        toml::Value::Boolean(b) => Walked::Scalar(b.to_string()),
        toml::Value::Float(f) => Walked::Scalar(f.to_string()),
        _ => Walked::NotScalar,
    }
}

fn walk_json(root: &serde_json::Value, anchor: &str) -> Walked {
    let mut cur = root;
    for seg in anchor.split('.') {
        match cur.get(seg) {
            Some(next) => cur = next,
            None => return Walked::MissingSegment(seg.to_string()),
        }
    }
    match cur {
        serde_json::Value::String(s) => Walked::Scalar(s.clone()),
        serde_json::Value::Number(n) => Walked::Scalar(n.to_string()),
        serde_json::Value::Bool(b) => Walked::Scalar(b.to_string()),
        _ => Walked::NotScalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write(dir: &TempDir, name: &str, body: &str) -> String {
        let p = dir.path().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn toml_scalar_leaf_resolves() {
        let d = TempDir::new().unwrap();
        let p = write(
            &d,
            "src.toml",
            "[requirements.r1]\ntitle = \"Alpha\"\nstatus = \"met\"\n",
        );
        let r = resolve_file_anchor(&p, "requirements.r1.title");
        assert_eq!(r.value.as_deref(), Some("Alpha"));
        assert!(r.reason.is_none());
        let s = resolve_file_anchor(&p, "requirements.r1.status");
        assert_eq!(s.value.as_deref(), Some("met"));
    }

    #[test]
    fn json_scalar_leaf_resolves() {
        let d = TempDir::new().unwrap();
        let p = write(&d, "src.json", r#"{"a":{"b":{"status":"unmet"}}}"#);
        let r = resolve_file_anchor(&p, "a.b.status");
        assert_eq!(r.value.as_deref(), Some("unmet"));
    }

    #[test]
    fn missing_file_yields_reason() {
        let r = resolve_file_anchor("/nope/does/not/exist.toml", "a.b");
        assert!(r.value.is_none());
        assert!(r.reason.unwrap().contains("cannot read file"));
    }

    #[test]
    fn missing_anchor_names_segment() {
        let d = TempDir::new().unwrap();
        let p = write(&d, "src.toml", "[a]\nx = 1\n");
        let r = resolve_file_anchor(&p, "a.b.c");
        assert!(r.value.is_none());
        let reason = r.reason.unwrap();
        assert!(reason.contains("missing segment 'b'"), "{reason}");
    }

    #[test]
    fn non_scalar_leaf_yields_reason() {
        let d = TempDir::new().unwrap();
        let p = write(&d, "src.toml", "[a]\nx = 1\n");
        let r = resolve_file_anchor(&p, "a");
        assert!(r.value.is_none());
        assert!(r.reason.unwrap().contains("not a scalar"));
    }

    #[test]
    fn command_exit_zero_and_nonzero() {
        let d = TempDir::new().unwrap();
        let dir = d.path().to_string_lossy().into_owned();
        let ok = run_command("exit 0", &dir);
        assert_eq!(ok.exit_code, Some(0));
        assert!(ok.reason.is_none());
        let bad = run_command("exit 3", &dir);
        assert_eq!(bad.exit_code, Some(3));
    }

    #[test]
    fn list_keys_returns_sorted_requirement_ids() {
        let d = TempDir::new().unwrap();
        let p = write(
            &d,
            "m.toml",
            "[requirements.R2]\ntitle = \"B\"\n[requirements.R1]\ntitle = \"A\"\n",
        );
        let keys = list_keys(&p, "requirements").unwrap();
        assert_eq!(keys, vec!["R1".to_string(), "R2".to_string()]);
        // missing prefix → Err naming the segment.
        assert!(list_keys(&p, "nope").is_err());
    }

    #[test]
    fn command_spawn_failure_yields_reason() {
        let r = run_command("true", "/nonexistent-dir-xyz");
        // Running in a missing dir fails to spawn.
        assert!(r.exit_code.is_none());
        assert!(r.reason.is_some());
    }
}
