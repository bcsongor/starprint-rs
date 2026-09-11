//! An HTTP server for print jobs, profiles and schedules.
//!
//! The command line and the desktop app each open a data directory and
//! run a [`Server`] on it, with bearer authentication, bound to loopback
//! by default, that prints its schedules on the local clock for as long
//! as it serves.

mod app;
mod body;
mod config;
mod data;
mod job;
mod printers;
mod problem;
mod schedule;
mod server;

pub use config::{Profile, ProfileSpec};
pub use data::Data;
pub use printers::{PrintQueue, reachable};
pub use schedule::{Due, ScheduleSpec, parse_cron};
pub use server::{DEFAULT_LISTEN, Server};
