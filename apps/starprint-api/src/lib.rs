//! An HTTP server for the desktop app's print jobs.
//!
//! The command line reads a profile file; the desktop app supplies its
//! profiles. Both run a [`Server`] with bearer authentication, bound to
//! loopback by default.

mod app;
mod body;
pub mod config;
mod job;
mod printers;
mod problem;
mod server;

pub use config::Profile;
pub use printers::PrintQueue;
pub use server::{DEFAULT_LISTEN, Server};
