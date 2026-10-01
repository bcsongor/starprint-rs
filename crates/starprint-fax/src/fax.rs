//! A fax as it travels: contents sealed to the recipient's X-Wing key
//! and signed by the sender's identity, so a relay sees two numbers, a
//! time and a size, and can neither read nor alter what it carries.
//!
//! A line's key is random and replaced every so often (see
//! [`FaxKey`]), so a fax can be opened only for as long as its recipient
//! keeps the key it was sealed to.
//!
//! Sealing is HPKE's one-shot shape by hand: X-Wing gives a fresh shared
//! secret per fax, HKDF-SHA256 turns it into a ChaCha20-Poly1305 key
//! bound to both numbers, and since that key seals one message, a zero
//! nonce is safe.

use chacha20poly1305::aead::{Aead as _, Payload};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit as _};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use x_wing::{Decapsulate as _, Encapsulate as _};

use crate::line::{FaxKey, Identity, Line, base64_bytes, transcript};
use crate::number::Number;

/// A sealed fax, as a relay holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fax {
    pub from: Number,
    pub to: Number,
    /// Unix seconds, by the sender's clock.
    pub sent: u64,
    /// X-Wing's ciphertext, from which the recipient recovers the key.
    #[serde(with = "base64_bytes")]
    pub encapsulated: Vec<u8>,
    #[serde(with = "base64_bytes")]
    pub sealed: Vec<u8>,
    /// The sender's, over everything above.
    #[serde(with = "base64_bytes")]
    pub signature: Vec<u8>,
}

impl Fax {
    /// Seals `contents` from `sender`, who answers on `from`, to `to`.
    /// What the contents mean is the server's business, not the
    /// protocol's.
    pub fn seal(sender: &Identity, from: Number, to: &Line, contents: &[u8], sent: u64) -> Self {
        let (encapsulated, shared) = to.fax_key.encapsulate();
        let encapsulated = encapsulated.to_vec();
        let sealed = cipher(&shared, from, to.number, &encapsulated)
            .encrypt(
                &Default::default(),
                Payload {
                    msg: contents,
                    aad: &associated(from, to.number, sent),
                },
            )
            .expect("ChaCha20-Poly1305 seals any length a fax can be");
        let mut fax = Self {
            from,
            to: to.number,
            sent,
            encapsulated,
            sealed,
            signature: Vec::new(),
        };
        fax.signature = sender.sign(&fax.signed());
        fax
    }

    fn signed(&self) -> Vec<u8> {
        transcript(
            "starprint fax v1",
            &[
                &self.from.to_string().into_bytes(),
                &self.to.to_string().into_bytes(),
                &self.sent.to_be_bytes(),
                &self.encapsulated,
                &self.sealed,
            ],
        )
    }

    /// Whether `sender`, the line this fax says it is from, signed it.
    /// A relay checks this before holding a fax, and the recipient
    /// again before opening one.
    pub fn signed_by(&self, sender: &Line) -> bool {
        sender.number == self.from && sender.identity.verify(&self.signed(), &self.signature)
    }

    /// What tells this fax from every other: a hash of its signature,
    /// in hex. A relay cannot change it without breaking the signature,
    /// so a recipient that remembers it knows a fax it has printed.
    pub fn id(&self) -> String {
        Sha256::digest(&self.signature)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// The contents, if this fax is for `number`, was sealed to `key`
    /// and `sender` signed it.
    pub fn open(&self, key: &FaxKey, number: Number, sender: &Line) -> Result<Vec<u8>, String> {
        if self.to != number {
            return Err(format!("the fax is for {}, not this line", self.to));
        }
        if !self.signed_by(sender) {
            return Err(format!("the fax is not signed by {}", self.from));
        }
        let encapsulated = x_wing::Ciphertext::try_from(self.encapsulated.as_slice())
            .map_err(|_| "the fax's encapsulated key is the wrong length")?;
        let shared = key.key().decapsulate(&encapsulated);
        cipher(&shared, self.from, self.to, &self.encapsulated)
            .decrypt(
                &Default::default(),
                Payload {
                    msg: &self.sealed,
                    aad: &associated(self.from, self.to, self.sent),
                },
            )
            .map_err(|_| "the fax does not open with this line's key".to_owned())
    }
}

fn cipher(shared: &[u8], from: Number, to: Number, encapsulated: &[u8]) -> ChaCha20Poly1305 {
    let info = transcript(
        "starprint fax key v1",
        &[
            &from.to_string().into_bytes(),
            &to.to_string().into_bytes(),
            encapsulated,
        ],
    );
    let mut key = [0; 32];
    Hkdf::<Sha256>::new(None, shared)
        .expand(&info, &mut key)
        .expect("32 bytes is a valid HKDF length");
    ChaCha20Poly1305::new(&key.into())
}

fn associated(from: Number, to: Number, sent: u64) -> Vec<u8> {
    transcript(
        "starprint fax contents v1",
        &[
            &from.to_string().into_bytes(),
            &to.to_string().into_bytes(),
            &sent.to_be_bytes(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::test_line;

    #[test]
    fn a_fax_opens_for_its_recipient_alone() {
        let (anna, anna_key, anna_record) = test_line("Anna");
        let (_, ben_key, ben_record) = test_line("Ben");
        let anna_line = anna_record.check().unwrap();
        let ben_line = ben_record.check().unwrap();

        let fax = Fax::seal(&anna, anna_line.number, &ben_line, b"Hello", 7);
        assert!(!fax.sealed.windows(5).any(|w| w == b"Hello"), "sealed");
        let json = serde_json::to_string(&fax).unwrap();
        let fax: Fax = serde_json::from_str(&json).unwrap();
        assert_eq!(
            fax.open(&ben_key, ben_line.number, &anna_line).unwrap(),
            b"Hello"
        );

        assert!(
            fax.open(&anna_key, anna_line.number, &anna_line).is_err(),
            "not for Anna"
        );
        assert!(
            fax.open(&ben_key, ben_line.number, &ben_line).is_err(),
            "not from Ben"
        );
        assert!(
            fax.open(&FaxKey::generate(), ben_line.number, &anna_line)
                .is_err(),
            "not with a key Ben has since replaced"
        );

        // Two faxes of the same words are still two faxes.
        let again = Fax::seal(&anna, anna_line.number, &ben_line, b"Hello", 7);
        assert_eq!(fax.id(), fax.clone().id());
        assert_ne!(fax.id(), again.id());
    }

    #[test]
    fn a_relay_cannot_alter_a_fax() {
        let (anna, _, anna_record) = test_line("Anna");
        let (_, ben_key, ben_record) = test_line("Ben");
        let anna_line = anna_record.check().unwrap();
        let ben_line = ben_record.check().unwrap();
        let fax = Fax::seal(&anna, anna_line.number, &ben_line, b"Hello", 7);

        let mut later = fax.clone();
        later.sent = 8;
        let mut flipped = fax.clone();
        flipped.sealed[0] ^= 1;
        for altered in [later, flipped] {
            assert!(!altered.signed_by(&anna_line));
            assert!(altered.open(&ben_key, ben_line.number, &anna_line).is_err());
        }
    }
}
