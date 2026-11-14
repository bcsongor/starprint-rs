//! The command line: a data directory, one server until it is told to
//! stop.

mod args;

use std::process::ExitCode;
use std::sync::Arc;

use starprint_api::{Data, Server};

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

    // Read before the runtime starts: a directory the server cannot
    // work from is a failure to start, not a request that fails later.
    let dir = args.data.map_or_else(Data::default_dir, Ok)?;
    let data = Arc::new(Data::open(&dir, args.token)?);
    let profiles = data.profiles();
    let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
    println!(
        "starprint-api: {} and {} in {}",
        if names.is_empty() {
            "no printers".to_owned()
        } else {
            names.join(", ")
        },
        match data.schedules().len() {
            1 => "1 schedule".to_owned(),
            n => format!("{n} schedules"),
        },
        dir.display()
    );

    tokio::runtime::Runtime::new()
        .map_err(|e| format!("the runtime could not start: {e}"))?
        .block_on(async {
            let server = Server::bind(args.listen, Arc::clone(&data), Arc::default(), true).await?;
            println!(
                "starprint-api: listening on http://{} with token {}",
                server.local_addr(),
                data.token()
            );
            stop_signal().await;
            server.shutdown().await
        })
}

/// Ctrl-C, or the `SIGTERM` a service manager sends, so a server
/// stopped by systemd drains its requests too.
async fn stop_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                () = interrupt => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    interrupt.await;
}
