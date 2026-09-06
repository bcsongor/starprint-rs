//! The command line.

use std::net::SocketAddr;
use std::path::PathBuf;

use starprint_api::DEFAULT_LISTEN;

pub const USAGE: &str = "\
starprint-api. Print starprint jobs over HTTP.

Usage: starprint-api [--config <path>] [--listen <addr>] [--token <value>]

  --config <path>  Printer profiles, in TOML (default: printers.toml)
  --listen <addr>  Address to bind (default: 127.0.0.1:9110)
  --token <value>  Bearer token every request must carry (default: generated)
  -h, --help       Print this message
";

const DEFAULT_CONFIG: &str = "printers.toml";

#[derive(Debug)]
pub struct Args {
    pub config: PathBuf,
    pub listen: SocketAddr,
    /// `None` means the caller wants one generated.
    pub token: Option<String>,
}

/// `None` when the caller asked for help and wants no work done.
pub fn parse(raw: impl IntoIterator<Item = String>) -> Result<Option<Args>, String> {
    let mut config = None;
    let mut listen = None;
    let mut token = None;
    let mut raw = raw.into_iter();
    while let Some(arg) = raw.next() {
        let mut value = |name: &str| {
            raw.next()
                .ok_or_else(|| format!("`{name}` needs a value; see --help"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--config" => config = Some(PathBuf::from(value("--config")?)),
            "--listen" => listen = Some(value("--listen")?),
            "--token" => token = Some(value("--token")?),
            other => return Err(format!("`{other}` is not an option; see --help")),
        }
    }

    let listen = match listen {
        Some(listen) => listen
            .parse()
            .map_err(|e| format!("`--listen {listen}` is not an address with a port: {e}"))?,
        None => DEFAULT_LISTEN,
    };
    if token.as_deref().is_some_and(|t| t.trim().is_empty()) {
        return Err("`--token` is empty".to_owned());
    }
    Ok(Some(Args {
        config: config.unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG)),
        listen,
        token,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &[&str]) -> Result<Option<Args>, String> {
        parse(raw.iter().map(|s| (*s).to_owned()))
    }

    #[test]
    fn no_arguments_gives_the_defaults() {
        let args = args(&[]).unwrap().expect("not help");
        assert_eq!(args.config, PathBuf::from(DEFAULT_CONFIG));
        assert_eq!(args.listen, DEFAULT_LISTEN);
        assert_eq!(args.token, None);
    }

    #[test]
    fn both_options_are_read() {
        let args = args(&[
            "--config",
            "p.toml",
            "--listen",
            "127.0.0.1:8080",
            "--token",
            "s3cret",
        ])
        .unwrap()
        .expect("not help");
        assert_eq!(args.config, PathBuf::from("p.toml"));
        assert_eq!(args.listen.to_string(), "127.0.0.1:8080");
        assert_eq!(args.token.as_deref(), Some("s3cret"));
    }

    #[test]
    fn help_asks_for_no_work() {
        assert!(args(&["--help"]).unwrap().is_none());
        assert!(args(&["-h"]).unwrap().is_none());
    }

    /// The default is loopback, but the caller decides.
    #[test]
    fn any_address_with_a_port_is_bound() {
        for address in ["0.0.0.0:9110", "192.168.1.10:80", "[::1]:9110"] {
            let args = args(&["--listen", address]).unwrap().expect("not help");
            assert_eq!(args.listen.to_string(), address);
        }
    }

    #[test]
    fn a_malformed_command_line_is_reported() {
        assert!(args(&["--config"]).unwrap_err().contains("needs a value"));
        assert!(args(&["--listen", "9110"]).unwrap_err().contains("address"));
        assert!(args(&["--token", " "]).unwrap_err().contains("empty"));
        assert!(
            args(&["--port", "1"])
                .unwrap_err()
                .contains("not an option")
        );
    }
}
