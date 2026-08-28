//! Round-trip test for the TCP transport against a local listener that
//! stands in for a printer's port-9100 interface.

use std::io::Read;
use std::net::TcpListener;
use std::thread;

use starprint::transport::TcpTransport;
use starprint::{Alignment, Cut};

#[test]
fn sends_document_bytes_verbatim() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut received = Vec::new();
        socket.read_to_end(&mut received).unwrap();
        received
    });

    let doc = starprint::starline()
        .align(Alignment::Center)
        .line("hello")
        .cut(Cut::FeedThenPartial)
        .build();

    let mut printer = TcpTransport::connect(&addr.to_string()).unwrap();
    printer.print(&doc).unwrap();
    drop(printer); // close the connection so the reader sees EOF

    assert_eq!(server.join().unwrap(), doc.as_bytes());
}
