use domain::ping;
use infrastructure::output::Output;
use std::io;

/// Execute the ping command through the output pipeline.
pub fn execute(output: &Output) -> io::Result<()> {
    let result = ping::execute();
    output.success("ping", &result, &mut io::stdout())
}
