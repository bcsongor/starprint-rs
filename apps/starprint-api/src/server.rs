//! Binding, serving and stopping.

use std::future::IntoFuture as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::app;
use crate::config::Profile;
use crate::printers::{PrintQueue, Printers};

/// Loopback, so only this machine can print.
pub const DEFAULT_LISTEN: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9110);

/// A server on a task of the current runtime. Dropping it stops it
/// without waiting; [`Server::shutdown`] waits for requests in flight.
pub struct Server {
    addr: SocketAddr,
    stop: oneshot::Sender<()>,
    done: JoinHandle<std::io::Result<()>>,
}

impl Server {
    /// Binds `listen` and serves `profiles` to callers with `token`.
    /// Share `queue` with other clients of these printers in this process.
    pub async fn bind(
        listen: SocketAddr,
        profiles: Vec<Profile>,
        token: String,
        queue: Arc<PrintQueue>,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind(listen)
            .await
            .map_err(|e| format!("{listen} could not be bound: {e}"))?;
        let addr = listener
            .local_addr()
            .map_err(|e| format!("{listen} has no address: {e}"))?;
        let (stop, stopped) = oneshot::channel();
        let router = app::router(Arc::new(Printers::new(profiles, queue)), token);
        let done = tokio::spawn(
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    // Resolves on `shutdown` and on drop alike.
                    let _ = stopped.await;
                })
                .into_future(),
        );
        Ok(Self { addr, stop, done })
    }

    /// Where it is listening, with the port the system chose if `bind`
    /// was given 0.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Stops accepting and returns once the requests in flight have
    /// finished, so a job half-written to a printer is not cut off.
    pub async fn shutdown(self) -> Result<(), String> {
        let _ = self.stop.send(());
        self.done
            .await
            .map_err(|e| format!("the server task failed: {e}"))?
            .map_err(|e| format!("the server stopped: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpStream;

    fn any_port() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)
    }

    async fn get_printers(addr: SocketAddr) -> String {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(
                b"GET /v1/printers HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer t\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).await.unwrap();
        reply
    }

    #[tokio::test]
    async fn serves_until_shut_down_and_then_releases_the_port() {
        let server = Server::bind(any_port(), Vec::new(), "t".to_owned(), Arc::default())
            .await
            .unwrap();
        let addr = server.local_addr();
        assert_ne!(addr.port(), 0);

        let reply = get_printers(addr).await;
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        assert!(reply.ends_with("[]"), "{reply}");

        server.shutdown().await.unwrap();
        assert!(TcpStream::connect(addr).await.is_err());
    }

    #[tokio::test]
    async fn a_port_in_use_is_reported_at_bind() {
        let first = Server::bind(any_port(), Vec::new(), "t".to_owned(), Arc::default())
            .await
            .unwrap();
        let error = Server::bind(
            first.local_addr(),
            Vec::new(),
            "t".to_owned(),
            Arc::default(),
        )
        .await
        .err()
        .expect("in use");
        assert!(error.contains("could not be bound"), "{error}");
    }
}
