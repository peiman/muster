//! `init` — stand up the store with zero ceremony (Manifesto #4). Idempotent.

use crate::store;
use crate::view::WithNext;
use infrastructure::output::Output;
use serde::Serialize;
use std::fmt;
use std::io;

#[derive(Serialize)]
struct InitResult {
    data_dir: String,
    schema_version: u32,
    initialized: bool,
}

impl fmt::Display for InitResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "initialized muster store at {} (schema v{})",
            self.data_dir, self.schema_version
        )
    }
}

pub fn execute(output: &Output) -> Result<(), Box<dyn std::error::Error>> {
    let dir = store::data_dir();
    store::init(&dir)?;
    let result = InitResult {
        data_dir: dir.display().to_string(),
        schema_version: 1,
        initialized: true,
    };
    let view = WithNext::new(
        &result,
        "muster process add <id> --name <name>  (then: muster explain)",
    );
    output.success("init", &view, &mut io::stdout())?;
    Ok(())
}
