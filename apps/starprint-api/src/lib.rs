//! A small HTTP server that prints the same jobs the desktop app does,
//! for other local programs to call.
//!
//! The binary in this crate runs one from a profile file until Ctrl-C.
//! The desktop app runs one behind a toggle, on the profiles it has.
//! Either way it is a [`Server`], which binds loopback by default and
//! takes a bearer token that every request must carry. The token is
//! the whole of the access control: a web page cannot know it, and a
//! caller on the network cannot print without it.

mod app;
mod body;
pub mod config;
mod job;
mod printers;
mod problem;
mod server;

pub use config::Profile;
pub use server::{DEFAULT_LISTEN, Server};
