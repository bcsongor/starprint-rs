//! Getting a [`Document`] to the printer. Implement [`Transport`] for USB,
//! serial or Bluetooth; [`TcpTransport`] covers port 9100.

use std::io::Write as _;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::Document;
use crate::error::{Error, Result};

/// Delivers bytes verbatim and in order.
pub trait Transport {
    fn send(&mut self, bytes: &[u8]) -> Result<()>;

    fn print(&mut self, document: &Document) -> Result<()> {
        self.send(document.as_bytes())
    }
}

/// Send `chunk_size` bytes, pause `delay`, repeat.
///
/// Star's Ethernet cards (IFBD-HE07/08) do not apply TCP back-pressure:
/// fed a large job at once, they drop data and the job never appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pacing {
    pub chunk_size: usize,
    pub delay: Duration,
}

impl Pacing {
    /// 1400 bytes every 20 ms (≈ 70 KB/s), the rate that never lost a job
    /// on an SP700 or TSP800II.
    pub const STAR_ETHERNET: Self = Self {
        chunk_size: 1400,
        delay: Duration::from_millis(20),
    };
}

/// Raw-socket printing on port 9100, paced by default.
///
/// ```no_run
/// use starprint::transport::TcpTransport;
///
/// let printer = TcpTransport::connect("192.168.1.60")?;
/// let printer = TcpTransport::connect("192.168.1.60:9100")?;
/// # Ok::<(), starprint::Error>(())
/// ```
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
    pacing: Option<Pacing>,
}

impl TcpTransport {
    pub const DEFAULT_PORT: u16 = 9100;

    /// For connects, reads and writes.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

    /// Connects to a host name or IP, with an optional `:port`.
    pub fn connect(host: &str) -> Result<Self> {
        // A bare IPv6 address has several colons, so a lone colon is only
        // a port separator outside brackets.
        let has_port = if host.starts_with('[') {
            host.contains("]:")
        } else {
            host.bytes().filter(|&b| b == b':').count() == 1
        };
        if has_port {
            Self::connect_addr(host)
        } else {
            // Strip `[...]` so `("::1", 9100)` resolves as an IPv6 host.
            let host = host
                .strip_prefix('[')
                .and_then(|h| h.strip_suffix(']'))
                .unwrap_or(host);
            Self::connect_addr((host, Self::DEFAULT_PORT))
        }
    }

    pub fn connect_addr(addr: impl ToSocketAddrs) -> Result<Self> {
        let mut last_err: Option<Error> = None;
        for addr in addr.to_socket_addrs().map_err(Error::Io)? {
            match TcpStream::connect_timeout(&addr, Self::DEFAULT_TIMEOUT) {
                Ok(stream) => {
                    stream.set_write_timeout(Some(Self::DEFAULT_TIMEOUT))?;
                    stream.set_read_timeout(Some(Self::DEFAULT_TIMEOUT))?;
                    stream.set_nodelay(true)?;
                    return Ok(Self {
                        stream,
                        pacing: Some(Pacing::STAR_ETHERNET),
                    });
                }
                Err(err) => last_err = Some(err.into()),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "address resolved to no socket addresses",
            ))
        }))
    }

    /// `None` blocks forever.
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.stream.set_write_timeout(timeout)?;
        Ok(())
    }

    /// `None` writes everything at once, which the older Star cards drop.
    pub fn set_pacing(&mut self, pacing: Option<Pacing>) {
        self.pacing = pacing;
    }

    /// [`Transport::print`] without the trait import.
    pub fn print(&mut self, document: &Document) -> Result<()> {
        Transport::print(self, document)
    }
}

impl Transport for TcpTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        match self.pacing {
            Some(Pacing { chunk_size, delay }) if chunk_size > 0 => {
                let mut chunks = bytes.chunks(chunk_size).peekable();
                while let Some(chunk) = chunks.next() {
                    self.stream.write_all(chunk)?;
                    self.stream.flush()?;
                    if chunks.peek().is_some() {
                        std::thread::sleep(delay);
                    }
                }
            }
            _ => {
                self.stream.write_all(bytes)?;
                self.stream.flush()?;
            }
        }
        Ok(())
    }
}
