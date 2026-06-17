//! `control import` — ingest a requirements manifest as **references**, not
//! copies (#7 Single Source of Truth; #10 the spec-as-hypothesis evolves with
//! its source). Each `<prefix>.<ID>` table becomes a control whose title/status
//! stay tied to the manifest via a `file_anchor` ref, so editing the manifest
//! changes muster's output with no muster mutation.

use crate::root::ManifestFormat;
use crate::store;
use domain::Ref;
use infrastructure::output::Output;
use infrastructure::resolver;
use serde::Serialize;
use std::fmt;
use std::io;
use std::path::Path;

type Boxed = Result<(), Box<dyn std::error::Error>>;

#[derive(Serialize)]
struct Imported {
    manifest: String,
    prefix: String,
    created: Vec<String>,
    skipped: Vec<Skipped>,
}

#[derive(Serialize)]
struct Skipped {
    id: String,
    reason: String,
}

impl fmt::Display for Imported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "imported {} control(s) as references from {} ({})",
            self.created.len(),
            self.manifest,
            self.prefix
        )?;
        for id in &self.created {
            writeln!(f, "  + {id}")?;
        }
        for s in &self.skipped {
            writeln!(f, "  ~ {} skipped: {}", s.id, s.reason)?;
        }
        Ok(())
    }
}

pub fn execute(
    dir: &Path,
    manifest: &str,
    format: Option<ManifestFormat>,
    prefix: &str,
    title_field: &str,
    output: &Output,
) -> Boxed {
    // `format` is advisory (the resolver infers TOML/JSON by extension); we only
    // validate that an explicit override matches the extension so a mismatch is
    // an honest error rather than a silent wrong-parser (#3).
    if let Some(fmt) = format {
        let is_json = manifest.ends_with(".json");
        let ok = matches!(
            (fmt, is_json),
            (ManifestFormat::Json, true) | (ManifestFormat::Toml, false)
        );
        if !ok {
            return Err(format!(
                "--format {fmt:?} does not match manifest extension of '{manifest}'"
            )
            .into());
        }
    }

    let ids = resolver::list_keys(manifest, prefix)?;
    let mut s = store::load(dir)?;
    let mut created = Vec::new();
    let mut skipped = Vec::new();

    for id in ids {
        let slug = slugify(&id);
        let anchor = format!("{prefix}.{id}.{title_field}");
        let r = Ref::FileAnchor {
            path: manifest.to_string(),
            anchor,
        };
        // Title is a fallback only; the ref is the authority. Use the slug as the
        // placeholder so a not-yet-resolved render is still legible.
        match s.add_control(&slug, &slug, None, true) {
            Ok(()) => {
                s.set_control_ref(&slug, r.clone())?;
                // #7 SSOT: imported controls are `file_anchor` refs that re-resolve
                // live on read — no authoritative `resolved` copy is persisted.
                created.push(slug);
            }
            Err(e) => skipped.push(Skipped {
                id: slug,
                reason: e.to_string(),
            }),
        }
    }

    store::save(dir, &s)?;
    let view = Imported {
        manifest: manifest.to_string(),
        prefix: prefix.to_string(),
        created,
        skipped,
    };
    output.success("control import", &view, &mut io::stdout())?;
    Ok(())
}

/// Lower-case + slug-normalize a manifest ID into a valid control id
/// (`CKSPEC-ARCH-001 → ckspec-arch-001`). Non-`[a-z0-9-]` runs collapse to `-`.
fn slugify(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut prev_dash = false;
    for ch in id.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    // Trim a trailing dash and ensure it starts with a letter (slug rule).
    let trimmed = out.trim_end_matches('-').to_string();
    match trimmed.chars().next() {
        Some(c) if c.is_ascii_lowercase() => trimmed,
        _ => format!("r-{trimmed}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_lowercases_and_hyphenates() {
        assert_eq!(slugify("CKSPEC-ARCH-001"), "ckspec-arch-001");
        assert_eq!(slugify("Req 1"), "req-1");
        assert_eq!(slugify("123-leading-digit"), "r-123-leading-digit");
    }
}
