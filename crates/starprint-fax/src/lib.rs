//! The fax protocol, which every starprint server and relay must agree
//! on: numbers that are the addresses of identity keys, the signed line
//! records relays hand out, sealed faxes, and the requests a line makes
//! of a relay.
//!
//! It does no I/O. The server's side lives in `starprint-api` and the
//! relay in `starprint-relay`; both speak through the types here.
//! Changing how a number is made, a transcript label or a field breaks
//! every line made or fax sent before it, so any of those is a new
//! protocol version.

mod fax;
mod line;
mod number;
mod relay;

pub use fax::Fax;
pub use line::{Identity, IdentityKey, Line, LineRecord, NAME_LIMIT, base64_bytes, now};
pub use number::Number;
pub use relay::{Action, Collect, Held, HeldFaxes, RelayInfo, check_relay_name};
