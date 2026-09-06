//! The command line: profiles from a file, one server until Ctrl-C.

mod args;

use std::process::ExitCode;

use starprint_api::{Server, config};

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
        .block_on(async {
            let token = args
                .token
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let server = Server::bind(args.listen, profiles, token.clone()).await?;
            println!(
                "starprint-api: listening on http://{} with token {token}",
                server.local_addr()
            );
            let _ = tokio::signal::ctrl_c().await;
            server.shutdown().await
        })
}
