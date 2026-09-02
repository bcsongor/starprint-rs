//! The printer profiles, read once at startup.
//!
//! Anything wrong with the file stops the server rather than surfacing
//! as a puzzling `400` later, so every check here reports the profile
//! and the field it is unhappy with. There is no API for managing
//! profiles; changing them means a restart.

use serde::Deserialize;
use starprint_workflows::{Paper, Printer, PrinterKind, Speed, check_density};

/// A named profile and the printer it builds. The desktop app hands
/// its own profiles over in this shape, already checked by their type,
/// so only the file goes through the checks below.
#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub name: String,
    pub printer: Printer,
}

/// `host`, `port`, `kind` and `paper` are fixed; the rest are defaults a
/// job request may override.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    host: String,
    port: u16,
    kind: PrinterKind,
    cut: bool,
    /// Thermal only, and required there: the roll width in millimetres.
    paper: Option<u16>,
    /// Thermal only, and required there.
    density: Option<i8>,
    /// Thermal only, and required there.
    speed: Option<Speed>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    printers: toml::Table,
}

/// Reads the file at `path`. The profiles keep the order the file lists
/// them in, so `GET /v1/printers` reads like the file.
pub fn read(path: &std::path::Path) -> Result<Vec<Profile>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse(text: &str) -> Result<Vec<Profile>, String> {
    let file: File = toml::from_str(text).map_err(|e| e.message().to_owned())?;
    file.printers
        .into_iter()
        .map(|(name, value)| {
            let raw = Raw::deserialize(value).map_err(|e| format!("{name}: {}", e.message()))?;
            profile(name, raw)
        })
        .collect()
}

fn profile(name: String, raw: Raw) -> Result<Profile, String> {
    if raw.host.trim().is_empty() {
        return Err(format!("{name}: `host` is empty"));
    }
    let mut printer = match raw.kind {
        PrinterKind::Thermal => {
            let mm = required(&name, "paper", raw.paper)?;
            let paper = Paper::from_mm(mm)
                .ok_or_else(|| format!("{name}: `paper` is {mm}, not 80 or 112"))?;
            let density = required(&name, "density", raw.density)?;
            check_density(density).map_err(|e| format!("{name}: {e}"))?;
            let speed = required(&name, "speed", raw.speed)?;
            Printer::thermal(raw.host, raw.port, paper, density, speed)
        }
        PrinterKind::Impact => {
            for (field, present) in [
                ("paper", raw.paper.is_some()),
                ("density", raw.density.is_some()),
                ("speed", raw.speed.is_some()),
            ] {
                if present {
                    return Err(format!(
                        "{name}: `{field}` is not a setting on an impact printer"
                    ));
                }
            }
            Printer::impact(raw.host, raw.port)
        }
    };
    printer.cut = raw.cut;
    Ok(Profile { name, printer })
}

fn required<T>(name: &str, field: &str, value: Option<T>) -> Result<T, String> {
    value.ok_or_else(|| format!("{name}: `{field}` is required for a thermal printer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use starprint_workflows::Head;

    const SAMPLE: &str = r#"
[printers.tsp800ii]
host = "192.168.1.180"
port = 9100
kind = "thermal"
paper = 80
cut = true
density = 3
speed = "slow"

[printers.sp743]
host = "192.168.1.141"
port = 9100
kind = "impact"
cut = true
"#;

    fn err(text: &str) -> String {
        parse(text).unwrap_err()
    }

    #[test]
    fn the_sample_file_reads_in_the_order_it_is_written() {
        let profiles = parse(SAMPLE).unwrap();
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["tsp800ii", "sp743"]);

        let thermal = &profiles[0].printer;
        assert_eq!(thermal.address(), "192.168.1.180:9100");
        assert_eq!(
            thermal.head,
            Head::Thermal {
                paper: Paper::Mm80,
                density: 3,
                speed: Speed::Slow
            }
        );
        assert!(thermal.cut);

        let impact = &profiles[1].printer;
        assert_eq!(impact.head, Head::Impact);
        assert_eq!(impact.address(), "192.168.1.141:9100");
    }

    #[test]
    fn a_file_without_printers_is_a_server_without_any() {
        assert!(parse("").unwrap().is_empty());
    }

    #[test]
    fn thermal_fields_are_required_on_a_thermal_printer() {
        let text = r#"
[printers.p]
host = "h"
port = 9100
kind = "thermal"
cut = true
density = 0
speed = "slow"
"#;
        assert_eq!(err(text), "p: `paper` is required for a thermal printer");
    }

    #[test]
    fn thermal_fields_are_refused_on_an_impact_printer() {
        let text = r#"
[printers.p]
host = "h"
port = 9100
kind = "impact"
cut = true
density = 3
"#;
        assert_eq!(
            err(text),
            "p: `density` is not a setting on an impact printer"
        );
    }

    #[test]
    fn invalid_values_are_reported_with_their_profile() {
        // A valid thermal profile with one field replaced.
        let with = |field: &str, value: &str| {
            let fields = [
                ("host", "\"h\""),
                ("port", "9100"),
                ("kind", "\"thermal\""),
                ("cut", "true"),
                ("paper", "80"),
                ("density", "0"),
                ("speed", "\"slow\""),
            ];
            let body: String = fields
                .iter()
                .map(|&(name, default)| {
                    let value = if name == field { value } else { default };
                    format!("{name} = {value}\n")
                })
                .collect();
            format!("[printers.p]\n{body}")
        };
        assert!(parse(&with("paper", "80")).is_ok(), "the profile is valid");
        assert_eq!(err(&with("paper", "58")), "p: `paper` is 58, not 80 or 112");
        assert_eq!(
            err(&with("density", "9")),
            "p: `density` is 9, outside -3 to 3"
        );
        assert!(err(&with("speed", "\"quick\"")).starts_with("p: "));
        assert!(err(&with("port", "70000")).starts_with("p: "));
    }

    #[test]
    fn an_unknown_field_stops_the_server() {
        let text = r#"
[printers.p]
host = "h"
port = 9100
kind = "impact"
cut = true
colour = true
"#;
        assert!(err(text).contains("colour"), "{}", err(text));
    }

    #[test]
    fn an_unknown_table_stops_the_server() {
        assert!(err("[server]\nport = 1\n").contains("server"));
    }

    #[test]
    fn an_empty_host_stops_the_server() {
        let text = "[printers.p]\nhost = \"  \"\nport = 9100\nkind = \"impact\"\ncut = true\n";
        assert_eq!(err(text), "p: `host` is empty");
    }
}
