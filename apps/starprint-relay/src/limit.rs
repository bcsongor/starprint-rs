//! Rate limits by client address: a bucket of tokens per address that
//! refills at a steady rate, so a burst goes through and a stream is
//! held to the rate. Faxes arrive as they come, so this is what keeps
//! one sender from filling a line's mailbox or its paper.
//!
//! An IPv6 client is counted by its /64. A network that size is handed
//! to one subscriber, who could otherwise take a new address for every
//! request.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Past this many addresses, full buckets are forgotten, since a full
/// bucket is the same as none.
const REMEMBERED: usize = 10_000;

pub struct Limit {
    burst: f64,
    per_second: f64,
    buckets: Mutex<HashMap<IpAddr, Bucket>>,
}

#[derive(Clone, Copy)]
struct Bucket {
    tokens: f64,
    at: Instant,
}

/// The address `client`'s bucket is kept under: its /64 for IPv6, and
/// the IPv4 address itself for one written as IPv6, which would
/// otherwise share a /64 with every other.
fn key(client: IpAddr) -> IpAddr {
    match client.to_canonical() {
        IpAddr::V6(address) => IpAddr::V6((u128::from(address) & (!0 << 64)).into()),
        v4 => v4,
    }
}

impl Limit {
    /// `burst` at once, then `per_hour`.
    pub fn new(burst: u32, per_hour: u32) -> Self {
        Self {
            burst: f64::from(burst),
            per_second: f64::from(per_hour) / 3600.0,
            buckets: Mutex::default(),
        }
    }

    fn filled(&self, bucket: Bucket, now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(bucket.at).as_secs_f64();
        (bucket.tokens + elapsed * self.per_second).min(self.burst)
    }

    /// Takes a token from `client`'s bucket, or answers how long until
    /// there is one.
    pub fn take(&self, client: IpAddr, now: Instant) -> Result<(), Duration> {
        let mut buckets = self.buckets.lock().unwrap();
        if buckets.len() >= REMEMBERED {
            buckets.retain(|_, bucket| self.filled(*bucket, now) < self.burst);
        }
        let bucket = buckets.entry(key(client)).or_insert(Bucket {
            tokens: self.burst,
            at: now,
        });
        let tokens = self.filled(*bucket, now);
        if tokens < 1.0 {
            return Err(Duration::from_secs_f64((1.0 - tokens) / self.per_second));
        }
        *bucket = Bucket {
            tokens: tokens - 1.0,
            at: now,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_goes_through_and_then_the_rate_holds() {
        let limit = Limit::new(3, 3600);
        let (anna, ben) = ("192.0.2.1".parse().unwrap(), "192.0.2.2".parse().unwrap());
        let start = Instant::now();
        for _ in 0..3 {
            limit.take(anna, start).unwrap();
        }
        let wait = limit.take(anna, start).unwrap_err();
        assert!(wait <= Duration::from_secs(1), "{wait:?}");
        limit.take(ben, start).unwrap();

        // One token a second comes back.
        limit.take(anna, start + Duration::from_secs(1)).unwrap();
        assert!(limit.take(anna, start + Duration::from_secs(1)).is_err());
    }

    #[test]
    fn an_ipv6_network_shares_one_bucket() {
        let limit = Limit::new(1, 1);
        let take = |client: &str| limit.take(client.parse().unwrap(), Instant::now());
        take("2001:db8:0:1::1").unwrap();
        assert!(take("2001:db8:0:1:ffff::2").is_err(), "the same /64");
        take("2001:db8:0:2::1").unwrap();

        // An IPv4 address written as IPv6 is still that address alone.
        take("::ffff:192.0.2.1").unwrap();
        assert!(take("192.0.2.1").is_err());
        take("::ffff:192.0.2.2").unwrap();
    }
}
