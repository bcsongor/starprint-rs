//! Fax numbers: ten digits mined from an identity key, so anyone can
//! check that a key is the one a number belongs to with a single hash
//! and without trusting whoever handed the key over.
//!
//! Mining runs Argon2id over the key and a counter until the hash starts
//! with `zero_bits` zero bits; the 33 bits after them are the number. A
//! forger has to hit the zero bits and one line's 33 bits with the same
//! hash, about 2^49 tries at the protocol's difficulty where the owner
//! needed about 2^16. It has to be one expensive hash: a cheap one for
//! the digits would let a forger filter on that first.

use std::fmt;
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use argon2::{Algorithm, Argon2, Block, Params, Version};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How many bits a number carries: ten digits, up to `*8589 934591`.
const BITS: u32 = 33;
const SALT: &[u8] = b"starprint fax number v1";

/// A line's number, shown as `*7441 720938`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Number(u64);

impl Number {
    /// The ten digits alone, as a URL carries them.
    pub fn digits(self) -> String {
        format!("{:010}", self.0)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = self.digits();
        write!(f, "*{} {}", &digits[..4], &digits[4..])
    }
}

impl FromStr for Number {
    type Err = String;

    /// Ten digits, with or without the star, spaces and dashes, since
    /// people type numbers the way they read them.
    fn from_str(text: &str) -> Result<Self, String> {
        let digits: String = text
            .trim()
            .trim_start_matches('*')
            .chars()
            .filter(|c| !matches!(c, ' ' | '-'))
            .collect();
        if digits.len() != 10 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("`{text}` is not a fax number; one has ten digits"));
        }
        let value: u64 = digits.parse().map_err(|e| format!("`{text}`: {e}"))?;
        if value >> BITS != 0 {
            return Err(format!(
                "`{text}` is not a fax number; none is above *8589 934591"
            ));
        }
        Ok(Self(value))
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

/// What mining a number costs, and so what forging one costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difficulty {
    memory_kib: u32,
    zero_bits: u32,
}

impl Difficulty {
    /// About 50 ms and 64 MiB a try and 2^16 tries on average: roughly
    /// seven minutes on an eight-core laptop, and about £100M of
    /// compute to forge one number. Every server and relay must agree
    /// on it, so changing it is a new protocol version.
    pub const PROTOCOL: Self = Self {
        memory_kib: 64 * 1024,
        zero_bits: 16,
    };

    /// Argon2's smallest memory and no zero bits, so a test mines in
    /// one try. A line mined at it is refused by anything checking at
    /// [`Self::PROTOCOL`], so it is only good for tests.
    pub const TEST: Self = Self {
        memory_kib: 8,
        zero_bits: 0,
    };

    /// The tries mining takes on average, for a progress bar.
    pub fn expected_tries(self) -> u64 {
        1 << self.zero_bits
    }

    fn argon2(self) -> Argon2<'static> {
        let params = Params::new(self.memory_kib, 1, 1, Some(32)).expect("valid Argon2 parameters");
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
    }

    /// The number this try gives, if it has the zero bits.
    fn try_counter(
        self,
        argon2: &Argon2<'_>,
        memory: &mut [Block],
        identity: &[u8],
        counter: u64,
    ) -> Option<Number> {
        let mut input = identity.to_vec();
        input.extend_from_slice(&counter.to_le_bytes());
        let mut hash = [0; 32];
        argon2
            .hash_password_into_with_memory(&input, SALT, &mut hash, memory)
            .expect("an identity key is a valid Argon2 input");
        let head = u64::from_be_bytes(hash[..8].try_into().unwrap());
        if self.zero_bits > 0 && head >> (64 - self.zero_bits) != 0 {
            return None;
        }
        Some(Number((head << self.zero_bits) >> (64 - BITS)))
    }

    fn memory(self) -> Vec<Block> {
        vec![Block::default(); self.memory_kib as usize]
    }

    /// Whether `counter` mines `number` from `identity`. One hash, so a
    /// few tens of milliseconds and 64 MiB at the protocol's difficulty:
    /// call it off the async runtime.
    pub fn check(self, identity: &[u8], counter: u64, number: Number) -> bool {
        self.try_counter(&self.argon2(), &mut self.memory(), identity, counter) == Some(number)
    }

    /// Tries counters on every core until one gives a number, counting
    /// tries in `tried`. Blocks for minutes at the protocol's
    /// difficulty. `None` if `stop` was set first.
    pub fn mine(
        self,
        identity: &[u8],
        tried: &AtomicU64,
        stop: &AtomicBool,
    ) -> Option<(u64, Number)> {
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get() as u64);
        let found = Mutex::new(None);
        let done = AtomicBool::new(false);
        std::thread::scope(|scope| {
            for first in 0..threads {
                let (found, done) = (&found, &done);
                scope.spawn(move || {
                    let argon2 = self.argon2();
                    let mut memory = self.memory();
                    let mut counter = first;
                    while !done.load(Ordering::Relaxed) && !stop.load(Ordering::Relaxed) {
                        if let Some(number) =
                            self.try_counter(&argon2, &mut memory, identity, counter)
                        {
                            done.store(true, Ordering::Relaxed);
                            found.lock().unwrap().get_or_insert((counter, number));
                        }
                        tried.fetch_add(1, Ordering::Relaxed);
                        counter += threads;
                    }
                });
            }
        });
        found.into_inner().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_reads_as_it_is_written() {
        let number: Number = "*7441 720938".parse().unwrap();
        assert_eq!(number.to_string(), "*7441 720938");
        for typed in ["7441720938", " *7441-720938 ", "*744 172 0938"] {
            assert_eq!(typed.parse::<Number>().unwrap(), number, "{typed}");
        }
        assert_eq!(
            "0000000042".parse::<Number>().unwrap().to_string(),
            "*0000 000042"
        );
        assert_eq!(
            serde_json::to_value(number).unwrap(),
            serde_json::json!("*7441 720938")
        );
    }

    #[test]
    fn anything_but_ten_digits_in_range_is_refused() {
        for bad in [
            "",
            "*7441 72093",
            "*7441 7209381",
            "*7441 72093x",
            "*9000 000000",
        ] {
            assert!(bad.parse::<Number>().is_err(), "{bad}");
        }
        assert!("*8589 934591".parse::<Number>().is_ok(), "the largest");
    }

    #[test]
    fn a_mined_number_checks_against_its_key_and_no_other() {
        let difficulty = Difficulty {
            memory_kib: 8,
            zero_bits: 4,
        };
        let (counter, number) = difficulty
            .mine(b"identity", &AtomicU64::new(0), &AtomicBool::new(false))
            .unwrap();
        assert!(difficulty.check(b"identity", counter, number));
        assert!(!difficulty.check(b"another", counter, number));
        assert!(!difficulty.check(b"identity", counter + 1, number));
    }

    #[test]
    fn a_stopped_miner_gives_up() {
        let stop = AtomicBool::new(true);
        assert!(
            Difficulty::PROTOCOL
                .mine(b"identity", &AtomicU64::new(0), &stop)
                .is_none()
        );
    }
}
