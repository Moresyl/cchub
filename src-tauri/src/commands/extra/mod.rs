use std::time::Instant;

pub(crate) fn log_command_timing(command: &str, started_at: Instant) {
    eprintln!(
        "[cchub][invoke] {command} completed in {}ms",
        started_at.elapsed().as_millis()
    );
}

mod activity_logs;
mod config_profiles;
mod custom_paths;
mod mcp_clients;
mod proxy_settings;
mod statusline;
mod types;
mod workspaces;
pub use activity_logs::*;
pub use config_profiles::*;
pub use custom_paths::*;
pub use mcp_clients::*;
pub use proxy_settings::*;
pub use statusline::*;
pub use types::*;
pub use workspaces::*;
