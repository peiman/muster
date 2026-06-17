use clap::{Parser, Subcommand, ValueEnum};

/// A production-ready Rust CLI built with muster.
#[derive(Parser, Debug)]
#[command(name = "muster", about)]
pub struct Cli {
    /// Output format: text (human-readable, default) or json (machine-readable).
    /// Explicit --output text overrides config json=true or CKELETIN_JSON=true.
    #[arg(long, global = true)]
    pub output: Option<OutputFormat>,

    /// Configuration file path
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Enable verbose output (debug level)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Disable the audit log file for this run (CKSPEC-OUT-004 audit
    /// logging is on by default; this overrides it for the current run)
    #[arg(long, global = true)]
    pub no_audit: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format selection (CKSPEC-OUT-002).
/// Matches ckeletin-go convention: --output text|json
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Check connectivity — returns pong
    Ping,
    /// Print the binary's build identity (version, commit, date, dirty)
    Version,
    /// Emit the machine-readable command catalog (CKSPEC-AGENT-006)
    Catalog,
}
