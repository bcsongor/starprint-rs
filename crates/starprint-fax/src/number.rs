//! Fax numbers: `*star1…`, the address of a line's identity key.
//!
//! A number is the first 16 bytes of SHA-256 over the identity key,
//! written in Bech32m under the prefix `star`, the way Bitcoin and
//! Cosmos write addresses: `*star1`, 26 characters of hash and 6 of
//! checksum, which catches any mistyped character. Anyone can check
//! that a key is the one a number belongs to with a single hash, so no
//! relay has to be trusted to say whose number is whose. Another key
//! with the same number takes about 2^128 tries to find, as hard as
//! breaking AES-128, the bar NIST sets for its lowest post-quantum
//! category.

use std::fmt;
use std::str::FromStr;

use bech32::primitives::decode::CheckedHrpstring;
use bech32::{Bech32m, Hrp};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

/// What every number starts with, before the `1`.
const PREFIX: &str = "star";
const LENGTH: usize = 16;
/// Hashed ahead of the key, so a number is a hash of nothing else.
const LABEL: &[u8] = b"starprint fax number v1\0";

/// A line's number, shown as `*star1…`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Number([u8; LENGTH]);

impl Number {
    /// The number of the line whose identity key is `identity`, as
    /// [`crate::IdentityKey::to_bytes`] writes it.
    pub fn of(identity: &[u8]) -> Self {
        let mut hash = Sha256::new();
        hash.update(LABEL);
        hash.update(identity);
        Self(hash.finalize()[..LENGTH].try_into().unwrap())
    }

    /// The number without its star, as a URL carries it.
    pub fn slug(self) -> String {
        let prefix = Hrp::parse(PREFIX).expect("a valid prefix");
        bech32::encode::<Bech32m>(prefix, &self.0).expect("16 bytes encode")
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "*{}", self.slug())
    }
}

impl FromStr for Number {
    type Err = String;

    /// A number with or without its star, in either case and with any
    /// spaces, since people paste numbers from anywhere and URLs carry
    /// them without a star.
    fn from_str(text: &str) -> Result<Self, String> {
        let invalid = |why: &str| format!("`{text}` is not a fax number; {why}");
        let address: String = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .trim_start_matches('*')
            .to_lowercase();
        let checked = CheckedHrpstring::new::<Bech32m>(&address).map_err(|_| {
            invalid(
                "one is *star1 and 32 letters and digits, and a checksum that does not hold \
                 means a character is mistyped",
            )
        })?;
        if checked.hrp().as_str() != PREFIX {
            return Err(invalid("one starts with *star1"));
        }
        let bytes: Vec<u8> = checked.byte_iter().collect();
        let bytes = bytes
            .try_into()
            .map_err(|_| invalid("it is the wrong length"))?;
        Ok(Self(bytes))
    }
}

impl Serialize for Number {
    fn serialize<S: Serializer>(&self, out: S) -> Result<S::Ok, S::Error> {
        out.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: Deserializer<'de>>(input: D) -> Result<Self, D::Error> {
        String::deserialize(input)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every machine must read the same number from the same key.
    #[test]
    fn a_key_gives_the_same_number_everywhere() {
        let number = Number::of(b"identity");
        assert_eq!(number.to_string(), "*star1hlyjardgkxyxstpe5agg32v74yymc4t5");
        assert_eq!(number.to_string().len(), 38);
    }

    #[test]
    fn a_number_reads_as_it_is_written() {
        let number = Number::of(b"identity");
        let written = number.to_string();
        assert_eq!(written.parse::<Number>().unwrap(), number);
        assert_eq!(number.slug(), written[1..]);
        for typed in [
            written[1..].to_owned(),
            format!("  {}  ", written.to_uppercase()),
            format!("{} {} {}", &written[..6], &written[6..10], &written[10..]),
        ] {
            assert_eq!(typed.parse::<Number>().unwrap(), number, "{typed}");
        }
        assert_eq!(
            serde_json::to_value(number).unwrap(),
            serde_json::json!(written)
        );
    }

    #[test]
    fn a_mistyped_character_is_caught() {
        let written = Number::of(b"identity").to_string();
        for at in 6..written.len() {
            let mut typo = written.clone().into_bytes();
            typo[at] = if typo[at] == b'q' { b'p' } else { b'q' };
            let typo = String::from_utf8(typo).unwrap();
            assert!(typo.parse::<Number>().is_err(), "{typo}");
        }
    }

    #[test]
    fn anything_but_a_star_address_is_refused() {
        let other = bech32::encode::<Bech32m>(Hrp::parse("cosmos").unwrap(), &[7; LENGTH]).unwrap();
        let long = bech32::encode::<Bech32m>(Hrp::parse(PREFIX).unwrap(), &[7; 20]).unwrap();
        for bad in ["", "*star1", "*7441 720938", &other, &long] {
            assert!(bad.parse::<Number>().is_err(), "{bad}");
        }
    }
}
