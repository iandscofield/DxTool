//! DxTool diagnostic engine.
//!
//! This crate holds everything that actually inspects the machine. It has no UI
//! code so the desktop app, a future command-line tool, the Claude MCP server and
//! the future boot environment can all share it.

pub mod fmt;
pub mod model;

#[cfg(windows)]
pub mod av;
#[cfg(windows)]
pub mod devices;
#[cfg(windows)]
pub mod events;
#[cfg(windows)]
pub mod fixes;
#[cfg(windows)]
pub mod health;
#[cfg(windows)]
pub mod history;
#[cfg(windows)]
pub mod inventory;
#[cfg(windows)]
pub mod machine;
#[cfg(windows)]
pub mod mcp;
#[cfg(windows)]
pub mod report;
#[cfg(windows)]
pub mod platform;
#[cfg(windows)]
pub mod update;
#[cfg(windows)]
pub mod upgrades;
#[cfg(windows)]
pub mod wmiutil;
