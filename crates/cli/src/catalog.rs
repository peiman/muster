//! `catalog` command — emits the machine-readable command catalog
//! (CKSPEC-AGENT-006), derived from the SAME clap command tree the parser uses
//! (`Cli::command()`). Because the catalog and the parser read one tree, the
//! catalog cannot drift from the actual command set — anti-drift is structural,
//! not tested-in.
//!
//! The `Catalog` *types* are the framework's shared schema (in the ckeletin
//! crate, re-exported via infrastructure); this file is the clap-specific walk,
//! which is the only implementation-dependent part (clap is cli-only by
//! architecture). It fills the optional fields clap can derive (short,
//! description, default, possible_values) on top of the required core.

use clap::{Arg, ArgAction, Command as ClapCommand, CommandFactory};
use infrastructure::catalog::{Catalog, CatalogCommand, CatalogFlag};
use infrastructure::output::Output;
use std::io;

use crate::root::Cli;

fn about_of(cmd: &ClapCommand) -> String {
    cmd.get_about().map(|s| s.to_string()).unwrap_or_default()
}

/// clap auto-adds `--help`/`--version`; those are universal, not part of the
/// application's own surface, so the catalog omits them.
fn is_help_or_version(arg: &Arg) -> bool {
    matches!(
        arg.get_action(),
        ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version
    )
}

fn to_flag(arg: &Arg) -> CatalogFlag {
    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy().into_owned())
        .collect();
    // Required-core normalization: a value-consuming flag vs a boolean switch.
    let takes_value = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
    // possible_values is only meaningful for value-taking enumerated flags.
    // clap reports ["true","false"] for boolean switches — noise an agent could
    // misread as `--verbose true`, so suppress it for switches.
    let possible_values = if takes_value {
        arg.get_possible_values()
            .iter()
            .map(|p| p.get_name().to_string())
            .collect()
    } else {
        Vec::new()
    };
    CatalogFlag {
        long: arg
            .get_long()
            .map(str::to_string)
            .unwrap_or_else(|| arg.get_id().as_str().to_string()),
        required: arg.is_required_set(),
        takes_value,
        short: arg.get_short().map(|c| c.to_string()),
        description: arg.get_help().map(|s| s.to_string()),
        default: (!defaults.is_empty()).then(|| defaults.join(",")),
        possible_values,
    }
}

/// A command's own (non-global, non-positional) flags.
fn command_flags(cmd: &ClapCommand) -> Vec<CatalogFlag> {
    cmd.get_arguments()
        .filter(|a| !a.is_global_set() && !a.is_positional() && !is_help_or_version(a))
        .map(to_flag)
        .collect()
}

fn walk(cmd: &ClapCommand) -> CatalogCommand {
    CatalogCommand {
        name: cmd.get_name().to_string(),
        description: about_of(cmd),
        flags: command_flags(cmd),
        commands: cmd.get_subcommands().map(walk).collect(),
    }
}

/// Derive the catalog from a clap command tree. Global flags are collected once
/// at the top level (not duplicated into each command).
pub fn build(root: &ClapCommand) -> Catalog {
    Catalog {
        name: root.get_name().to_string(),
        description: about_of(root),
        global_flags: root
            .get_arguments()
            .filter(|a| a.is_global_set() && !is_help_or_version(a))
            .map(to_flag)
            .collect(),
        commands: root.get_subcommands().map(walk).collect(),
    }
}

/// Execute the `catalog` command through the output pipeline.
pub fn execute(output: &Output) -> io::Result<()> {
    let catalog = build(&Cli::command());
    output.success("catalog", &catalog, &mut io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_matches_the_command_tree() {
        let catalog = build(&Cli::command());

        // Name is non-empty and derived from the command (rename-stable: a
        // derived project renames it, so we don't hardcode the scaffold name).
        assert!(!catalog.name.is_empty());

        // Every subcommand the parser knows is in the catalog — including
        // `catalog` itself (self-referential).
        let names: Vec<&str> = catalog.commands.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"ping"));
        assert!(names.contains(&"version"));
        assert!(names.contains(&"catalog"));

        // Required-core normalization: --output consumes a value, --verbose is a switch.
        let output = catalog
            .global_flags
            .iter()
            .find(|f| f.long == "output")
            .expect("output is a global flag");
        assert!(output.takes_value);
        let verbose = catalog
            .global_flags
            .iter()
            .find(|f| f.long == "verbose")
            .expect("verbose is a global flag");
        assert!(!verbose.takes_value);

        // --output is enumerated → optional possible_values derived.
        assert!(output.possible_values.contains(&"text".to_string()));

        // clap's auto --help is excluded.
        assert!(!catalog.global_flags.iter().any(|f| f.long == "help"));
    }
}
