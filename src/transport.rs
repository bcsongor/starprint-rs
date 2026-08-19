//! Transports that carry rendered documents to a printer.
//!
//! The protocol layer ([`Document`]) is pure: it only
//! produces bytes. Getting those bytes to the printer is the job of a
//! [`Transport`]. This module ships an Ethernet transport ([`TcpTransport`])
//! speaking the raw-socket printing protocol on TCP port 9100, which every
//! networked Star printer supports out of the box.
//!
//! Implement [`Transport`] yourself to add other links (USB, serial,
//! Bluetooth SPP) without touching the protocol layer.

use std::io::Write as _;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::Document;
use crate::error::{Error, Result};

/// A byte-oriented link to a printer.
///
/// Implementations must deliver the bytes verbatim and in order; the
/// protocol layer has already rendered every command.
pub trait Transport {
    /// Sends `bytes` to the printer, blocking until the transport has
    /// accepted all of them.
    fn send(&mut self, bytes: &[u8]) -> Result<()>;

    /// Sends a rendered [`Document`]; equivalent to
    /// [`send`](Self::send) with [`Document::as_bytes`].
    fn print(&mut self, document: &Document) -> Result<()> {
        self.send(document.as_bytes())
    }
}

/// An Ethernet transport using raw-socket ("port 9100") printing.
///
/// All networked Star printers accept print data on TCP port 9100 with no
/// framing or handshake: bytes written to the socket go straight to the
/// printer's input buffer.
///
/// # Examples
///
/// ```no_run
/// use starprint::transport::TcpTransport;
///
/// // Default port 9100:
/// let printer = TcpTransport::connect("192.168.1.60")?;
/// // Or an explicit address:
/// let printer = TcpTransport::connect("192.168.1.60:9100")?;
/// # Ok::<(), starprint::Error>(())
/// ```
#[derive(Debug)]
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// The raw-socket printing port used by Star network interfaces.
    pub const DEFAULT_PORT: u16 = 9100;

    /// Default timeout applied to connects, reads and writes.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

    /// Connects to a printer given a host name or IP address, with an
    /// optional `:port` suffix (defaults to port 9100).
    ///
    /// The connection and subsequent writes use [`Self::DEFAULT_TIMEOUT`];
    /// use [`set_write_timeout`](Self::set_write_timeout) to change it.
    pub fn connect(host: &str) -> Result<Self> {
        // `1.2.3.4:9100` and `[::1]:9100` already carry a port. A bare IPv6
        // address contains multiple colons, so a lone colon is only a port
        // separator in the un-bracketed forms.
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

    /// Connects to an explicit socket address (any [`ToSocketAddrs`] value,
    /// e.g. `("192.168.1.60", 9100)`).
    pub fn connect_addr(addr: impl ToSocketAddrs) -> Result<Self> {
        let mut last_err: Option<Error> = None;
        for addr in addr.to_socket_addrs().map_err(Error::Io)? {
            match TcpStream::connect_timeout(&addr, Self::DEFAULT_TIMEOUT) {
                Ok(stream) => {
                    stream.set_write_timeout(Some(Self::DEFAULT_TIMEOUT))?;
                    stream.set_read_timeout(Some(Self::DEFAULT_TIMEOUT))?;
                    stream.set_nodelay(true)?;
                    return Ok(Self { stream });
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

    /// Sets the write timeout for subsequent sends (`None` blocks forever).
    pub fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.stream.set_write_timeout(timeout)?;
        Ok(())
    }

    /// Sends a rendered [`Document`] to the printer.
    ///
    /// Inherent mirror of [`Transport::print`], so the common case needs
    /// no trait import.
    pub fn print(&mut self, document: &Document) -> Result<()> {
        Transport::print(self, document)
    }
}

impl Transport for TcpTransport {
    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()?;
        Ok(())
    }
}
