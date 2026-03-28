/// Minimal logger shim for headless-export.
///
/// The shared `src-tauri/src/plot3d.rs` module references `crate::logger`,
/// but for CLI export we only need no-op logging hooks.

pub fn log_info(_message: &str) {}

pub fn log_error(_message: &str) {}

pub fn log_debug(_message: &str) {}

pub fn log_entry(_level: &str, _message: &str, _module: Option<String>) {}
