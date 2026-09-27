//! The fax relay's command line: a name, an address and a database
//! file, one relay until Ctrl-C. Put it behind a TLS proxy on a public host; servers reach it
//! at its `https://` address.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use starprint_fax::{Difficulty, check_relay_name};

const USAGE: &str = "\
starprint-relay. Hold starprint faxes until their line polls for them.

Usage: starprint-relay --name <NAME> [--listen <addr>] [--db <file>]

  --name <NAME>    What servers call this relay, in capitals, digits and
                   dashes, like LONRELAY01
  --listen <addr>  Address to bind (default: 0.0.0.0:9120)
  --db <file>      SQLite file for lines and faxes, created if missing
                   (default: relay.db)
  -h, --help       Print this message
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("starprint-relay: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut name = None;
    let mut listen: SocketAddr = ([0, 0, 0, 0], 9120).into();
    let mut db = PathBuf::from("relay.db");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| {
            args.next()
                .ok_or_else(|| format!("`{flag}` needs a value; see --help"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "--name" => name = Some(value("--name")?),
            "--listen" => {
                let text = value("--listen")?;
                listen = text
                    .parse()
                    .map_err(|e| format!("`--listen {text}` is not an address with a port: {e}"))?;
            }
            "--db" => db = value("--db")?.into(),
            other => return Err(format!("`{other}` is not an option; see --help")),
        }
    }
    let name = name.ok_or("`--name` is required; see --help")?;
    check_relay_name(&name)?;
    let router = starprint_relay::router(name.clone(), Difficulty::PROTOCOL, &db)?;

    tokio::runtime::Runtime::new()
        .map_err(|e| format!("the runtime could not start: {e}"))?
        .block_on(async {
            let listener = tokio::net::TcpListener::bind(listen)
                .await
                .map_err(|e| format!("{listen} could not be bound: {e}"))?;
            println!("starprint-relay: {name} listening on http://{listen}");
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await
                .map_err(|e| format!("the relay stopped: {e}"))
        })
}
