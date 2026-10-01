// Proxy + visible apps + preferences + session scanners/parsers + tauri command handlers.
mod codex_titles;
mod commands;
mod native_sessions;
mod opencode_sessions;
mod preferences;
mod session_management;
mod session_parsers;
mod session_scanners;
pub(crate) mod session_tasks;
mod session_trash;

pub use commands::*;
pub use native_sessions::*;
pub use opencode_sessions::*;
pub use preferences::*;
pub use session_management::*;
pub use session_parsers::*;
pub use session_scanners::*;

#[cfg(test)]
mod archive_tests;
