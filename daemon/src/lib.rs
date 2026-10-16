//! Testable core for the Windows tray daemon.
//!
//! The domain/application modules do not depend on the GUI or Windows APIs.

pub mod application;
pub mod config;
pub mod domain;
pub mod http;
pub mod security;

#[cfg(windows)]
pub mod adapters;
