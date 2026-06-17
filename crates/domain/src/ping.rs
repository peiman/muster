use serde::Serialize;
use std::fmt;

/// Result of a ping operation.
/// Lives in domain — no framework imports. Enforced by Cargo.toml.
#[derive(Debug, Serialize, PartialEq)]
pub struct PingResult {
    pub message: String,
}

impl fmt::Display for PingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pong! {}", self.message)
    }
}

/// Execute ping. Pure function, no side effects.
pub fn execute() -> PingResult {
    PingResult {
        message: "muster is alive".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_result() {
        let result = execute();
        assert_eq!(result.message, "muster is alive");
    }

    #[test]
    fn ping_display_includes_pong() {
        assert_eq!(format!("{}", execute()), "Pong! muster is alive");
    }

    #[test]
    fn ping_serializes() {
        let json = serde_json::to_value(execute()).unwrap();
        assert_eq!(json["message"], "muster is alive");
    }
}
