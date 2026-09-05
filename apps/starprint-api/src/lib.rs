//! A small HTTP server that prints the same jobs the desktop app does,
//! for other local programs to call.
//!
//! The binary in this crate runs one from a profile file until Ctrl-C.
//! The desktop app runs one behind a toggle, on the profiles it has.
//! Either way it is a [`Server`], which binds loopback by default and
//! refuses requests made from web pages, so a site the user visits
//! cannot print through it. It has no authentication, so binding an
//! address the network can reach hands every printer it knows to
//! anyone on that network.

mod app;
mod body;
pub mod config;
mod job;
mod printers;
mod problem;
mod server;

pub use config::Profile;
pub use server::{DEFAULT_LISTEN, Server};
