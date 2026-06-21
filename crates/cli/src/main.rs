//! Entry point — bootstrap only (CKSPEC-ARCH-006).
//! All logic lives in domain and infrastructure crates.

mod catalog;
mod control;
mod explain;
mod import;
mod incident;
mod init;
mod nonconformity;
mod ping;
mod process;
mod readiness;
mod resolve;
mod root;
mod store;
mod version;
mod view;

use infrastructure::{
    config::Config,
    logging::{self, LogConfig, LogGuard},
    output::{Output, OutputMode},
};

/// Success: READY / gate passed, or no `--require-ready` requested.
pub(crate) const EXIT_OK: i32 = 0;
/// Command error — missing/uninitialized store, bad `--process` scope, IO,
/// config/log init. Rendered as the CKSPEC error envelope.
pub(crate) const EXIT_ERROR: i32 = 1;
// NOTE: exit code 2 is reserved by clap for usage errors (`parse_args` ->
// `Error::exit` -> `safe_exit(2)`); clap emits it BEFORE `run()` executes, so we
// never produce it here. The gate code MUST avoid 2 so a typo'd flag (2) and a
// not-ready gate (3) remain distinct CI signals.
/// `--require-ready` gate not met: a SUCCESSFUL readiness computation that did not
/// meet the bar (the full output is still rendered). Distinct from `EXIT_OK` (0),
/// `EXIT_ERROR` (1), and clap's usage code (2). Documented in `readiness --help`
/// and the README.
pub(crate) const EXIT_GATE_NOT_MET: i32 = 3;

/// The success outcome of a command dispatch. Makes illegal exit states
/// unrepresentable: a successful command can only resolve to `Ok` (0) or the
/// readiness gate's `GateNotMet` (3) — never clap's 2 or the error code 1.
/// `code()` is the SSOT numeric mapping, delegating to the exit constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// READY / gate passed / no gate requested.
    Ok,
    /// `readiness --require-ready` ran successfully but did not meet the bar.
    GateNotMet,
}

impl Outcome {
    pub(crate) fn code(self) -> i32 {
        match self {
            Outcome::Ok => EXIT_OK,
            Outcome::GateNotMet => EXIT_GATE_NOT_MET,
        }
    }
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    // Parse CLI args first — we need to know the output format
    // before we can route errors correctly.
    let cli = parse_args();

    // Capture the subcommand name BEFORE moving `cli` into run_inner,
    // so the error envelope can identify which subcommand failed
    // (CKSPEC-OUT-003). Earned 2026-04-22 — prior versions hardcoded
    // "init", producing `{"command":"init"}` for every failing
    // subcommand regardless of which one was running.
    let cmd_name = subcommand_name(&cli.command);

    // Stash the explicit CLI flag before moving cli, so error rendering
    // can reapply the same precedence rules as the inner function.
    let explicit_output = cli.output.clone();

    match run_inner(cli) {
        Ok((_guard, code)) => {
            // _guard holds the LogGuard for its lifetime here, ensuring the
            // audit worker flushes when run() returns. `code` is `outcome.code()`:
            // EXIT_OK for every command except a `readiness --require-ready` gate
            // miss (EXIT_GATE_NOT_MET). It can never be EXIT_ERROR or clap's 2 —
            // the `Outcome` type makes those unrepresentable on the success path.
            code
        }
        Err(RunError::PreConfig { error }) => {
            // Config load failed — output mode is CLI flag only (no config yet).
            let json_mode = resolve_output_mode(&explicit_output, false);
            let output = Output::new(if json_mode {
                OutputMode::Json
            } else {
                OutputMode::Human
            });
            let _ = output.error(
                cmd_name,
                &error.to_string(),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            );
            EXIT_ERROR
        }
        Err(RunError::LogInitFailed {
            error,
            json_mode: resolved_json_mode,
        }) => {
            // Logging init failed after config load. json_mode is known from
            // config; no guard exists so the error event won't be shadow-logged
            // (inherent: the audit infrastructure itself failed to start).
            let json_mode = resolve_output_mode(&explicit_output, resolved_json_mode);
            let output = Output::new(if json_mode {
                OutputMode::Json
            } else {
                OutputMode::Human
            });
            let _ = output.error(
                cmd_name,
                &error.to_string(),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            );
            EXIT_ERROR
        }
        Err(RunError::PostConfig {
            guard: _guard,
            error,
            json_mode: resolved_json_mode,
        }) => {
            // CKSPEC-OUT-002: errors in JSON mode MUST be JSON envelopes on stdout.
            // Errors in human mode go to stderr.
            // CKSPEC-OUT-003: the envelope MUST identify the failing subcommand.
            //
            // The guard is held here through error rendering, so the audit worker
            // is alive when Output::error emits its shadow-log event (CKSPEC-OUT-004).
            // Guard drops when this match arm's scope ends — AFTER output.error completes.
            let json_mode = resolve_output_mode(&explicit_output, resolved_json_mode);
            let output = Output::new(if json_mode {
                OutputMode::Json
            } else {
                OutputMode::Human
            });
            let _ = output.error(
                cmd_name,
                &error.to_string(),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            );
            EXIT_ERROR
        }
    }
}

/// Errors from run_inner, carrying the LogGuard when available so the
/// outer run() can render the error while the audit worker is still alive.
enum RunError {
    /// Error occurred before config was loaded (e.g., config file not found).
    /// The guard is not yet available; output mode is CLI-flag-only.
    PreConfig { error: Box<dyn std::error::Error> },
    /// Logging initialization failed (config was loaded but the log file
    /// couldn't be created). json_mode is known from config; no guard exists.
    LogInitFailed {
        error: Box<dyn std::error::Error>,
        /// Resolved json_mode (flag OR config.json) for the error renderer.
        json_mode: bool,
    },
    /// Error occurred after logging was initialized. The guard must be held
    /// through error rendering so the output.error shadow-log event reaches
    /// the audit file (CKSPEC-OUT-004).
    PostConfig {
        guard: LogGuard,
        error: Box<dyn std::error::Error>,
        /// Resolved json_mode (flag OR config.json) for the error renderer.
        json_mode: bool,
    },
}

/// Resolve the final output mode from the explicit CLI flag and config/env json flag.
///
/// Precedence (SSOT — computed ONCE here, not separately in run and run_inner):
/// 1. Explicit `--output text` → human (overrides config/env in both directions).
/// 2. Explicit `--output json` → JSON.
/// 3. `config_json` (from config file or CKELETIN_JSON env) → JSON.
/// 4. Default → human.
fn resolve_output_mode(explicit: &Option<root::OutputFormat>, config_json: bool) -> bool {
    match explicit {
        Some(root::OutputFormat::Json) => true,
        Some(root::OutputFormat::Text) => false, // explicit text overrides config/env
        None => config_json,
    }
}

/// Parse args, injecting the runtime build-identity line as clap's `--version`
/// output. Keeps `BuildInfo::version_line()` the single formatter (SSOT) while
/// `--version` surfaces the baked commit/date/dirty. clap's `get_matches()` and
/// `Error::exit` handle `--version` / `--help` / parse errors by exiting.
fn parse_args() -> root::Cli {
    use clap::{CommandFactory, FromArgMatches};
    let matches = root::Cli::command()
        .version(version::current().version_line())
        .get_matches();
    match root::Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    }
}

/// Map a parsed `Commands` variant to its CLI-visible name. A plain
/// `match` so adding a new subcommand is a compile error here until a
/// name is assigned — no silent "init" fallback. Consumers of ckeletin
/// extend this alongside their own `root::Commands` additions.
fn subcommand_name(command: &root::Commands) -> &'static str {
    match command {
        root::Commands::Init => "init",
        root::Commands::Explain => "explain",
        root::Commands::Process(_) => "process",
        root::Commands::Control(_) => "control",
        root::Commands::Incident(_) => "incident",
        root::Commands::Nonconformity(_) => "nonconformity",
        root::Commands::Readiness(_) => "readiness",
        root::Commands::Ping => "ping",
        root::Commands::Version => "version",
        root::Commands::Catalog => "catalog",
    }
}

/// Inner run. On success returns `(LogGuard, ())` — the guard is returned to
/// the caller so it outlives the entire run() scope. On failure returns
/// `RunError`, which for post-config failures also carries the guard so the
/// outer run() can render the error while the audit worker is still alive
/// (fixing the CKSPEC-OUT-004 gap where error events were dropped because the
/// guard died before Output::error ran).
fn run_inner(cli: root::Cli) -> Result<(LogGuard, i32), RunError> {
    // Load configuration (defaults → file → env). On failure, json_mode is
    // unknown (no config yet), so we use CLI-flag-only mode.
    let config = Config::load(cli.config.as_deref(), "MUSTER_")
        .map_err(|e| RunError::PreConfig { error: e })?;

    // Determine output mode ONCE (SSOT). Explicit CLI flag wins both directions;
    // config.json (set via config file or CKELETIN_JSON env) is the fallback.
    let json_mode = resolve_output_mode(&cli.output, config.json);

    // Determine log level: --verbose overrides config
    let log_level = if cli.verbose {
        "debug".to_string()
    } else {
        config.log_level.clone()
    };

    // Audit log (CKSPEC-OUT-004) is on by default; --no-audit turns it off for
    // this run. The path is resolved to a stable per-user location (default
    // ~/.config/<app>/logs/app.log) so it doesn't depend on the cwd.
    let audit_enabled = config.log_file_enabled && !cli.no_audit;
    let audit_path = logging::resolve_audit_path(
        &config.log_file_path,
        &config.log_location,
        env!("CARGO_BIN_NAME"),
    );

    // First-run heads-up: tell the user once — when the audit log directory is
    // first created — that we're writing it and how to turn it off. Goes to the
    // status stream (stderr), human mode only; silent in JSON mode and on every
    // later run.
    //
    // Note: this eprintln! intentionally runs BEFORE logging::init (the audit
    // worker isn't alive yet — the notice is about creating the log dir), so it
    // is not shadow-logged. This is the one justified pre-audit stderr write.
    if audit_enabled && !json_mode {
        let audit_dir = audit_path.parent();
        let first_run = audit_dir.is_some_and(|dir| !dir.as_os_str().is_empty() && !dir.exists());
        if first_run {
            // The daily rolling appender appends a YYYY-MM-DD suffix to the
            // filename, so the actual log files are named e.g. "app.log.2026-06-09".
            // We name the directory (which is stable) and the filename pattern.
            let log_file_name = audit_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "app.log".to_string());
            let dir_display = audit_dir
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| audit_path.display().to_string());
            eprintln!(
                "note: writing audit logs under {dir_display}/ (files named {log_file_name}.<date>); \
                 this notice won't repeat; disable with --no-audit or log_file_enabled=false in config"
            );
        }
    }

    // Initialize logging — suppress stderr in JSON mode for clean output.
    // A logging init failure happens AFTER config load, so json_mode is
    // known. We don't have a guard yet (init failed), but we know the
    // correct json_mode for the error envelope. Map to PreConfig but
    // with json_mode captured via explicit_output for the outer handler.
    //
    // NOTE: For logging init failures, the guard doesn't exist, so the
    // error envelope's shadow-log event will NOT reach an audit file
    // (no worker is running). This is inherent — the audit infrastructure
    // failed to start — and documented as acceptable.
    let log_config = LogConfig {
        console_level: if json_mode {
            "off".to_string()
        } else {
            log_level
        },
        file_enabled: audit_enabled,
        file_path: audit_path.to_string_lossy().into_owned(),
        file_level: config.log_file_level.clone(),
    };
    // Use a temporary LogGuard-shaped error that carries json_mode so
    // the outer handler can render the error in the correct mode even
    // though no actual guard was created.
    let guard = logging::init(&log_config).map_err(|e| RunError::LogInitFailed {
        error: e,
        json_mode,
    })?;

    let output = Output::new(if json_mode {
        OutputMode::Json
    } else {
        OutputMode::Human
    });

    // Dispatch to command handler. On failure, wrap the error as PostConfig so
    // the caller receives the guard alongside the error — keeping the audit
    // worker alive through error rendering (CKSPEC-OUT-004 fix).
    // Each arm yields an `Outcome`: the readiness handler returns its gate
    // outcome (Outcome::Ok or Outcome::GateNotMet), every other command maps a
    // successful `Ok(())` to Outcome::Ok. The outcome's `code()` is carried out
    // of the dispatch so run() can return it verbatim — keeping the exit-code
    // policy in the cli (not domain). The `Outcome` type makes it impossible for
    // a successful command to forge clap's 2 or the error code 1.
    let dispatch_result: Result<Outcome, Box<dyn std::error::Error>> = match cli.command {
        root::Commands::Init => init::execute(&output).map(|()| Outcome::Ok),
        root::Commands::Explain => explain::execute(&output).map(|()| Outcome::Ok),
        root::Commands::Process(c) => process::execute(c.sub, &output).map(|()| Outcome::Ok),
        root::Commands::Control(c) => control::execute(c.sub, &output).map(|()| Outcome::Ok),
        root::Commands::Incident(c) => incident::execute(c.sub, &output).map(|()| Outcome::Ok),
        root::Commands::Nonconformity(c) => {
            nonconformity::execute(c.sub, &output).map(|()| Outcome::Ok)
        }
        root::Commands::Readiness(a) => readiness::execute(a, &output),
        root::Commands::Ping => ping::execute(&output)
            .map(|()| Outcome::Ok)
            .map_err(|e| Box::new(e) as _),
        root::Commands::Version => version::execute(&output)
            .map(|()| Outcome::Ok)
            .map_err(|e| Box::new(e) as _),
        root::Commands::Catalog => catalog::execute(&output)
            .map(|()| Outcome::Ok)
            .map_err(|e| Box::new(e) as _),
    };

    match dispatch_result {
        Ok(outcome) => Ok((guard, outcome.code())),
        Err(error) => Err(RunError::PostConfig {
            guard,
            error,
            json_mode,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{EXIT_GATE_NOT_MET, EXIT_OK, Outcome};

    /// Pin the numeric exit-code contract that `Outcome::code()` is the SSOT for:
    /// Ok => 0 (gate passed / no gate), GateNotMet => 3 (ran but did not meet the
    /// bar). A future edit to `code()` that breaks the documented contract is
    /// caught here.
    #[test]
    fn outcome_code_is_pinned() {
        assert_eq!(Outcome::Ok.code(), EXIT_OK); // 0
        assert_eq!(Outcome::GateNotMet.code(), EXIT_GATE_NOT_MET); // 3
    }
}
