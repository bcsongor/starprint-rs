//! An HTTP server for print jobs, profiles, schedules and faxes.
//!
//! The command line and the desktop app each open a data directory and
//! run a [`Server`] on it, with bearer authentication, bound to loopback
//! by default, that prints its schedules on the local clock and the
//! faxes its relays hold for as long as it serves. The fax protocol is
//! `starprint-fax`'s, and the relays are `starprint-relay`.

mod app;
mod body;
mod config;
mod data;
mod faxing;
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
