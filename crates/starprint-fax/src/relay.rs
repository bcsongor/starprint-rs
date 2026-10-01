//! What a server and a relay say to each other, beyond records and
//! faxes: the relay's name, the signed requests a line makes to collect
//! its faxes, and the faxes a relay hands back.

use serde::{Deserialize, Serialize};

use crate::fax::Fax;
use crate::line::{Identity, Line, base64_bytes, now, transcript};
use crate::number::Number;

/// What `GET /v1/relay` answers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayInfo {
    pub name: String,
    pub version: String,
}

/// A relay's name: capitals, digits and dashes, as people read them out.
pub fn check_relay_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 32
        || !name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(format!(
            "`{name}` is not a relay name; use up to 32 capitals, digits and dashes, like LONRELAY01"
        ));
    }
    Ok(())
}

/// What a [`Collect`] asks for, which its signature covers so a poll
/// cannot be replayed as a confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// See the faxes held for the line.
    Poll,
    /// Clear faxes that have printed.
    Confirm,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Poll => "poll",
            Self::Confirm => "confirm",
        }
    }
}

/// A request to see or clear the faxes held for a line, signed by the
/// line's identity so only its owner can. `ids` is empty for a poll.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Collect {
    /// Unix seconds, so a relay can refuse an old one.
    pub at: u64,
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(with = "base64_bytes")]
    pub signature: Vec<u8>,
}

impl Collect {
    /// A poll for the faxes held for `number`.
    pub fn poll(identity: &Identity, number: Number) -> Self {
        Self::new(Action::Poll, identity, number, Vec::new())
    }

    /// Clears `ids`, once their faxes have printed.
    pub fn confirm(identity: &Identity, number: Number, ids: Vec<String>) -> Self {
        Self::new(Action::Confirm, identity, number, ids)
    }

    fn new(action: Action, identity: &Identity, number: Number, ids: Vec<String>) -> Self {
        let at = now();
        Self {
            signature: identity.sign(&Self::signed(action, number, at, &ids)),
            at,
            ids,
        }
    }

    fn signed(action: Action, number: Number, at: u64, ids: &[String]) -> Vec<u8> {
        let ids = ids.join(",");
        transcript(
            &format!("starprint fax {} v1", action.label()),
            &[
                &number.to_string().into_bytes(),
                &at.to_be_bytes(),
                ids.as_bytes(),
            ],
        )
    }

    /// Whether `line` signed this as an `action` on its own number. The
    /// relay checks `at` against its clock itself.
    pub fn signed_by(&self, action: Action, line: &Line) -> bool {
        line.identity.verify(
            &Self::signed(action, line.number, self.at, &self.ids),
            &self.signature,
        )
    }
}

/// A fax as a relay holds it, under the id it gave it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Held {
    pub id: String,
    pub fax: Fax,
}

/// What a poll answers.
#[derive(Debug, Serialize, Deserialize)]
pub struct HeldFaxes {
    pub faxes: Vec<Held>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::test_line;

    #[test]
    fn a_name_reads_like_one() {
        assert!(check_relay_name("LONRELAY01").is_ok());
        assert!(check_relay_name("NYC-2").is_ok());
        for bad in ["", "lonrelay01", "LON RELAY", &"A".repeat(33)] {
            assert!(check_relay_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn only_the_line_collects_and_only_as_it_asked() {
        let (anna, _, anna_record) = test_line("Anna");
        let (ben, _, _) = test_line("Ben");
        let line = anna_record.check().unwrap();

        let poll = Collect::poll(&anna, line.number);
        assert!(poll.signed_by(Action::Poll, &line));
        assert!(!poll.signed_by(Action::Confirm, &line), "not a confirm");
        assert!(!Collect::poll(&ben, line.number).signed_by(Action::Poll, &line));

        let mut confirm = Collect::confirm(&anna, line.number, vec!["a".to_owned()]);
        assert!(confirm.signed_by(Action::Confirm, &line));
        confirm.ids.push("b".to_owned());
        assert!(!confirm.signed_by(Action::Confirm, &line), "ids are signed");
    }
}
