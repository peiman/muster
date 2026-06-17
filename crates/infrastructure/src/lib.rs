// Re-export framework modules — project code imports from infrastructure, not ckeletin
pub use ckeletin::build_info;
pub use ckeletin::catalog;
pub use ckeletin::config;
pub use ckeletin::logging;
pub use ckeletin::output;
pub use ckeletin::process;

// muster v1 — the dereference engine (the disk/process boundary, #8). Infra-local
// result types; the cli bridges them to domain::Resolution.
pub mod resolver;
