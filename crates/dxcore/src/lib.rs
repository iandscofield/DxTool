//! DxTool diagnostic engine.
//!
//! This crate holds everything that actually inspects the machine. It has no UI
//! code so the desktop app, a future command-line tool, the Claude MCP server and
//! the future boot environment can all share it.

pub mod fmt;
pub mod model;

#[cfg(windows)]
pub mod devices;
#[cfg(windows)]
pub mod inventory;
#[cfg(windows)]
pub mod platform;
#[cfg(windows)]
pub mod wmiutil;
