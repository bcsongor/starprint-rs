//! A fax line: the identity that owns a number, and the signed record a
//! relay hands out so that others can check the number and seal faxes
//! to it.
//!
//! Everything is hybrid, so a line holds as long as either half does:
//! the identity signs with Ed25519 and ML-DSA-65, and faxes are sealed to
//! an X-Wing key (X25519 and ML-KEM-768) that the identity signs.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::Signer as _;
use hkdf::Hkdf;
use ml_dsa::{EncodedSignature, EncodedVerifyingKey, KeyExport as _, MlDsa65};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::number::{Difficulty, Number};

const ED25519_KEY: usize = 32;
const ED25519_SIGNATURE: usize = 64;
/// ML-DSA's context string, so a signature made here means nothing
/// anywhere else.
const CONTEXT: &[u8] = b"starprint fax";
/// Long enough for a person and their printer, short enough for a line
/// on paper.
pub const NAME_LIMIT: usize = 64;

/// A line's secret: one seed, from which every key is derived, so the
/// data directory keeps 32 bytes and nothing else.
pub struct Identity {
    seed: [u8; 32],
    ed25519: ed25519_dalek::SigningKey,
    ml_dsa: ml_dsa::SigningKey<MlDsa65>,
    fax: x_wing::DecapsulationKey,
}

impl Identity {
    pub fn generate() -> Self {
        let mut seed = [0; 32];
        getrandom::fill(&mut seed).expect("the system has randomness");
        Self::from_seed(seed)
    }

    pub fn from_seed(seed: [u8; 32]) -> Self {
        let derive = |label: &[u8]| {
            let mut key = [0; 32];
            Hkdf::<Sha256>::new(None, &seed)
                .expand(label, &mut key)
                .expect("32 bytes is a valid HKDF length");
            key
        };
        Self {
            seed,
            ed25519: ed25519_dalek::SigningKey::from_bytes(&derive(b"starprint fax ed25519")),
            ml_dsa: ml_dsa::SigningKey::from_seed(&derive(b"starprint fax ml-dsa-65").into()),
            fax: x_wing::DecapsulationKey::from(derive(b"starprint fax x-wing")),
        }
    }

    pub fn seed(&self) -> [u8; 32] {
        self.seed
    }

    pub fn key(&self) -> IdentityKey {
        IdentityKey {
            ed25519: self.ed25519.verifying_key(),
            ml_dsa: self.ml_dsa.expanded_key().verifying_key(),
        }
    }

    /// Both signatures over `message`, Ed25519's first.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let mut signature = self.ed25519.sign(message).to_bytes().to_vec();
        let ml_dsa = self
            .ml_dsa
            .expanded_key()
            .sign_deterministic(message, CONTEXT)
            .expect("the context fits");
        signature.extend_from_slice(&ml_dsa.encode());
        signature
    }

    pub fn fax_key(&self) -> &x_wing::DecapsulationKey {
        &self.fax
    }

    /// This line's record, signed at `updated`.
    pub fn record(&self, number: Number, counter: u64, name: &str, updated: u64) -> LineRecord {
        use x_wing::Decapsulator as _;
        let mut record = LineRecord {
            number,
            identity: self.key().to_bytes(),
            counter,
            fax_key: self.fax.encapsulation_key().to_bytes().to_vec(),
            name: name.to_owned(),
            updated,
            signature: Vec::new(),
        };
        record.signature = self.sign(&record.signed());
        record
    }
}

/// The public half of an [`Identity`]: what a number is mined from and
/// what checks a line's signatures.
#[derive(Clone)]
pub struct IdentityKey {
    ed25519: ed25519_dalek::VerifyingKey,
    ml_dsa: ml_dsa::VerifyingKey<MlDsa65>,
}

impl PartialEq for IdentityKey {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

impl IdentityKey {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.ed25519.to_bytes().to_vec();
        bytes.extend_from_slice(&self.ml_dsa.encode());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let (ed25519, ml_dsa) = bytes
            .split_at_checked(ED25519_KEY)
            .ok_or("the identity key is too short")?;
        let ed25519 = ed25519_dalek::VerifyingKey::from_bytes(ed25519.try_into().unwrap())
            .map_err(|e| format!("the identity's Ed25519 key is invalid: {e}"))?;
        let ml_dsa = EncodedVerifyingKey::<MlDsa65>::try_from(ml_dsa)
            .map_err(|_| "the identity's ML-DSA key is the wrong length")?;
        Ok(Self {
            ed25519,
            ml_dsa: ml_dsa::VerifyingKey::decode(&ml_dsa),
        })
    }

    /// Both halves must hold.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        let Some((ed25519, ml_dsa)) = signature.split_at_checked(ED25519_SIGNATURE) else {
            return false;
        };
        let ed25519 = ed25519_dalek::Signature::from_bytes(ed25519.try_into().unwrap());
        let Some(ml_dsa) = EncodedSignature::<MlDsa65>::try_from(ml_dsa)
            .ok()
            .and_then(|encoded| ml_dsa::Signature::decode(&encoded))
        else {
            return false;
        };
        self.ed25519.verify_strict(message, &ed25519).is_ok()
            && self.ml_dsa.verify_with_context(message, CONTEXT, &ml_dsa)
    }
}

/// What a relay stores and hands out for a number. It checks itself:
/// the number against the identity and counter, the rest against the
/// identity's signature. Of two records for a number, the one updated
/// last wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineRecord {
    pub number: Number,
    #[serde(with = "base64_bytes")]
    pub identity: Vec<u8>,
    pub counter: u64,
    /// The X-Wing key faxes to this line are sealed to.
    #[serde(with = "base64_bytes")]
    pub fax_key: Vec<u8>,
    /// Who answers, as its owner put it: shown before a first fax, but
    /// not checked by anyone.
    pub name: String,
    /// Unix seconds.
    pub updated: u64,
    #[serde(with = "base64_bytes")]
    pub signature: Vec<u8>,
}

impl LineRecord {
    fn signed(&self) -> Vec<u8> {
        transcript(
            "starprint fax line v1",
            &[
                &self.number.to_string().into_bytes(),
                &self.identity,
                &self.counter.to_be_bytes(),
                &self.fax_key,
                self.name.as_bytes(),
                &self.updated.to_be_bytes(),
            ],
        )
    }

    /// The keys, once the number, the name and the signature hold. Runs
    /// one Argon2 hash, so call it off the async runtime.
    ///
    /// `pinned` is the identity this number had when it was first seen,
    /// if it has been. Another identity is refused, and the same one
    /// skips the hash, since its number was checked then.
    pub fn check(&self, difficulty: Difficulty, pinned: Option<&[u8]>) -> Result<Line, String> {
        if pinned.is_some_and(|pinned| pinned != self.identity) {
            return Err(format!(
                "{} has a different key from the one it had when first seen; it may not be who \
                 it says it is",
                self.number
            ));
        }
        if self.name.chars().count() > NAME_LIMIT {
            return Err(format!("the name is longer than {NAME_LIMIT} characters"));
        }
        let identity = IdentityKey::from_bytes(&self.identity)?;
        if !identity.verify(&self.signed(), &self.signature) {
            return Err("the line's signature does not hold".to_owned());
        }
        let fax_key = x_wing::EncapsulationKey::try_from(self.fax_key.as_slice())
            .map_err(|_| "the line's fax key is invalid")?;
        if pinned.is_none() && !difficulty.check(&self.identity, self.counter, self.number) {
            return Err(format!("{} was not mined from this identity", self.number));
        }
        Ok(Line {
            number: self.number,
            identity,
            fax_key,
            name: self.name.clone(),
        })
    }
}

/// A [`LineRecord`] that has been checked, with its keys decoded.
#[derive(Clone)]
pub struct Line {
    pub number: Number,
    pub identity: IdentityKey,
    pub fax_key: x_wing::EncapsulationKey,
    pub name: String,
}

impl std::fmt::Debug for Line {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Line")
            .field("number", &self.number)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Unix seconds, the clock records and faxes are dated by.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The bytes a signature covers: a label naming what is signed, then
/// each part with its length, so no two different messages read alike.
pub fn transcript(label: &str, parts: &[&[u8]]) -> Vec<u8> {
    let mut bytes = label.as_bytes().to_vec();
    bytes.push(0);
    for part in parts {
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    bytes
}

/// Bytes as a base64 string, for keys, signatures and sealed faxes: a
/// serde `with` module.
pub mod base64_bytes {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], out: S) -> Result<S::Ok, S::Error> {
        out.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(input: D) -> Result<Vec<u8>, D::Error> {
        STANDARD
            .decode(String::deserialize(input)?)
            .map_err(serde::de::Error::custom)
    }

    /// The same for an `Option`, absent when `None`.
    pub mod option {
        use super::*;

        pub fn serialize<S: Serializer>(
            bytes: &Option<Vec<u8>>,
            out: S,
        ) -> Result<S::Ok, S::Error> {
            match bytes {
                Some(bytes) => super::serialize(bytes, out),
                None => out.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            input: D,
        ) -> Result<Option<Vec<u8>>, D::Error> {
            Option::<String>::deserialize(input)?
                .map(|text| STANDARD.decode(text))
                .transpose()
                .map_err(serde::de::Error::custom)
        }
    }
}

/// A new line mined at the test difficulty, with its record.
#[cfg(test)]
pub(crate) fn test_line(name: &str) -> (Identity, LineRecord) {
    use std::sync::atomic::{AtomicBool, AtomicU64};
    let identity = Identity::generate();
    let (counter, number) = Difficulty::TEST
        .mine(
            &identity.key().to_bytes(),
            &AtomicU64::new(0),
            &AtomicBool::new(false),
        )
        .unwrap();
    let record = identity.record(number, counter, name, 1);
    (identity, record)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_come_back_from_the_seed() {
        let identity = Identity::generate();
        let again = Identity::from_seed(identity.seed());
        assert!(identity.key() == again.key());
        let key = IdentityKey::from_bytes(&identity.key().to_bytes()).unwrap();
        assert!(key == identity.key());
    }

    #[test]
    fn both_signatures_must_hold() {
        let identity = Identity::generate();
        let signature = identity.sign(b"hello");
        assert!(identity.key().verify(b"hello", &signature));
        assert!(!identity.key().verify(b"hullo", &signature));
        for byte in [0, signature.len() - 1] {
            let mut broken = signature.clone();
            broken[byte] ^= 1;
            assert!(!identity.key().verify(b"hello", &broken), "{byte}");
        }
        assert!(!Identity::generate().key().verify(b"hello", &signature));
    }

    #[test]
    fn a_record_checks_itself() {
        let (_, record) = test_line("Anna");
        let json = serde_json::to_string(&record).unwrap();
        let back: LineRecord = serde_json::from_str(&json).unwrap();
        let line = back.check(Difficulty::TEST, None).unwrap();
        assert_eq!(line.number, record.number);
        assert_eq!(line.name, "Anna");

        let mut renamed = record.clone();
        renamed.name = "Mallory".to_owned();
        assert!(renamed.check(Difficulty::TEST, None).is_err(), "signed");

        // A relay swapping in its own identity has to re-sign, and then
        // the number no longer comes from the key.
        let (_, theirs) = test_line("Anna");
        let forged = Identity::generate().record(record.number, theirs.counter, "Anna", 2);
        let error = forged.check(Difficulty::TEST, None).unwrap_err();
        assert!(error.contains("was not mined"), "{error}");
        let error = forged
            .check(Difficulty::TEST, Some(&record.identity))
            .unwrap_err();
        assert!(error.contains("different key"), "{error}");
        assert!(
            record
                .check(Difficulty::TEST, Some(&record.identity))
                .is_ok()
        );
    }
}
