//! What a server and a relay say to each other, beyond records and
//! faxes: the relay's name, the signed requests a line makes to collect
//! its faxes, and the faxes a relay hands back.

use serde::{Deserialize, Serialize};

use crate::fax::Fax;
use crate::line::{Identity, Line, base64_bytes, transcript};
use crate::number::Number;

/// How long a relay holds a fax nobody polls for: thirty days, in
/// seconds. A line keeps a fax key for as long and a day over, so both
/// sides read the one figure.
pub const HOLD_SECONDS: u64 = 30 * 24 * 60 * 60;
/// The sealed bytes a relay holds for one line before it turns faxes
/// away. A poll answers with all of them, so a server sizes what it
/// reads of an answer from this.
pub const HELD_BYTES: usize = 64 << 20;

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
    /// The label its transcript is signed under.
    fn label(self) -> &'static str {
        match self {
            Self::Poll => "starprint fax poll v1",
            Self::Confirm => "starprint fax confirm v1",
        }
    }
}

/// A request to see or clear the faxes held for a line, signed by the
/// line's identity so only its owner can. `ids` is empty for a poll.
/// The signature names the relay asked, so one relay cannot pass a
/// request on to another and collect or clear what that one holds.
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
    /// A poll of the relay called `relay` for the faxes held for
    /// `number`, signed at `at`.
    pub fn poll(identity: &Identity, relay: &str, number: Number, at: u64) -> Self {
        Self::new(Action::Poll, identity, relay, number, Vec::new(), at)
    }

    /// Clears `ids` at the relay called `relay`, once their faxes have
    /// printed. Signed at `at`.
    pub fn confirm(
        identity: &Identity,
        relay: &str,
        number: Number,
        ids: Vec<String>,
        at: u64,
    ) -> Self {
        Self::new(Action::Confirm, identity, relay, number, ids, at)
    }

    fn new(
        action: Action,
        identity: &Identity,
        relay: &str,
        number: Number,
        ids: Vec<String>,
        at: u64,
    ) -> Self {
        Self {
            signature: identity.sign(&Self::signed(action, relay, number, at, &ids)),
            at,
            ids,
        }
    }

    fn signed(action: Action, relay: &str, number: Number, at: u64, ids: &[String]) -> Vec<u8> {
        let ids = ids.join(",");
        transcript(
            action.label(),
            &[
                relay.as_bytes(),
                &number.to_string().into_bytes(),
                &at.to_be_bytes(),
                ids.as_bytes(),
            ],
        )
    }

    /// Whether `line` signed this as an `action` on its own number at
    /// the relay called `relay`. The relay checks `at` against its clock
    /// itself.
    pub fn signed_by(&self, action: Action, relay: &str, line: &Line) -> bool {
        line.identity.verify(
            &Self::signed(action, relay, line.number, self.at, &self.ids),
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

    /// The labels are protocol, so they are written out here as every
    /// line made so far signs them.
    #[test]
    fn a_request_is_signed_under_its_own_label() {
        let number = Number::of(b"identity");
        let signed = |action| Collect::signed(action, "LON", number, 7, &[]);
        assert!(signed(Action::Poll).starts_with(b"starprint fax poll v1\0"));
        assert!(signed(Action::Confirm).starts_with(b"starprint fax confirm v1\0"));
    }

    #[test]
    fn only_the_line_collects_and_only_as_it_asked() {
        let (anna, _, anna_record) = test_line();
        let (ben, _, _) = test_line();
        let line = anna_record.check().unwrap();

        let poll = Collect::poll(&anna, "LON", line.number, 7);
        assert!(poll.signed_by(Action::Poll, "LON", &line));
        assert!(
            !poll.signed_by(Action::Confirm, "LON", &line),
            "not a confirm"
        );
        assert!(
            !poll.signed_by(Action::Poll, "NYC", &line),
            "not for that relay"
        );
        assert!(!Collect::poll(&ben, "LON", line.number, 7).signed_by(Action::Poll, "LON", &line));

        let mut confirm = Collect::confirm(&anna, "LON", line.number, vec!["a".to_owned()], 7);
        assert!(confirm.signed_by(Action::Confirm, "LON", &line));
        confirm.ids.push("b".to_owned());
        assert!(
            !confirm.signed_by(Action::Confirm, "LON", &line),
            "ids are signed"
        );
    }
}
