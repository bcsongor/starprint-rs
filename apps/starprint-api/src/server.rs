//! Binding, serving and stopping.

use std::future::IntoFuture as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::app;
use crate::data::Data;
use crate::printers::{PrintQueue, Printers};
use crate::schedule;

/// Loopback, so only this machine can print.
pub const DEFAULT_LISTEN: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9110);

/// A server on a task of the current runtime, running the schedules in
/// its data for as long as it serves. Dropping it stops it without
/// waiting; [`Server::shutdown`] waits for requests in flight, not for
/// scheduled jobs.
pub struct Server {
    addr: SocketAddr,
    stop: oneshot::Sender<()>,
    done: JoinHandle<std::io::Result<()>>,
}

impl Server {
    /// Binds `listen` and serves `data` to callers with its token.
    /// Share `queue` with other clients of these printers in this process.
    pub async fn bind(
        listen: SocketAddr,
        data: Arc<Data>,
        queue: Arc<PrintQueue>,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind(listen)
            .await
            .map_err(|e| format!("{listen} could not be bound: {e}"))?;
        let addr = listener
            .local_addr()
            .map_err(|e| format!("{listen} has no address: {e}"))?;
        let (stop, stopped) = oneshot::channel();
        let printers = Arc::new(Printers::new(data, queue));
        let router = app::router(Arc::clone(&printers));
        let done = tokio::spawn(async move {
            let (drain, draining) = oneshot::channel();
            let serve = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = draining.await;
                })
                .into_future();
            tokio::pin!(serve);
            // Stop scheduling and cancel queued scheduled jobs before
            // waiting for HTTP requests to drain. A blocking write
            // already in progress keeps its queue guard until it ends.
            {
                let scheduler = schedule::serve(printers);
                tokio::pin!(scheduler);
                tokio::select! {
                    result = &mut serve => return result,
                    _ = stopped => {},
                    () = &mut scheduler => {},
                }
            }
            let _ = drain.send(());
            serve.await
        });
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

    fn empty() -> Arc<Data> {
        Arc::new(Data::ephemeral(Vec::new(), "t".to_owned()))
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
        let server = Server::bind(any_port(), empty(), Arc::default())
            .await
            .unwrap();
        let addr = server.local_addr();
        assert_ne!(addr.port(), 0);

        let reply = get_printers(addr).await;
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        assert!(reply.ends_with("\"printers\":[]}"), "{reply}");

        server.shutdown().await.unwrap();
        assert!(TcpStream::connect(addr).await.is_err());
    }

    #[tokio::test]
    async fn a_port_in_use_is_reported_at_bind() {
        let first = Server::bind(any_port(), empty(), Arc::default())
            .await
            .unwrap();
        let error = Server::bind(first.local_addr(), empty(), Arc::default())
            .await
            .err()
            .expect("in use");
        assert!(error.contains("could not be bound"), "{error}");
    }
}
