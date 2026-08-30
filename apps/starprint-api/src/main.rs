//! A small HTTP server that prints the same jobs the desktop app does,
//! for other local programs to call.
//!
//! It binds the loopback interface by default and enables no CORS. It
//! has no authentication, so `--listen` on an address the network can
//! reach hands every printer in the profile file to anyone who asks.

mod app;
mod args;
mod body;
mod config;
mod job;
mod printers;
mod problem;

use std::net::SocketAddr;
use std::process::ExitCode;
use std::sync::Arc;

use config::Profile;
use printers::Printers;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("starprint-api: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let Some(args) = args::parse(std::env::args().skip(1))? else {
        print!("{}", args::USAGE);
        return Ok(());
    };

    // Read before the runtime starts: a file the server cannot work
    // from is a failure to start, not a request that fails later.
    let profiles = config::read(&args.config)?;
    let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
    println!(
        "starprint-api: {} from {}",
        if names.is_empty() {
            "no printers".to_owned()
        } else {
            names.join(", ")
        },
        args.config.display()
    );

    tokio::runtime::Runtime::new()
        .map_err(|e| format!("the runtime could not start: {e}"))?
        .block_on(serve(args.listen, profiles))
}

async fn serve(listen: SocketAddr, profiles: Vec<Profile>) -> Result<(), String> {
    let router = app::router(Arc::new(Printers::new(profiles)));
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .map_err(|e| format!("{listen} could not be bound: {e}"))?;
    println!("starprint-api: listening on http://{listen}");
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| format!("the server stopped: {e}"))
}
