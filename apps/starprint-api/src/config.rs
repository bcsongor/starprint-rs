//! The printer profiles: the file they live in and the shape a client
//! sends one in.
//!
//! The file is read once at startup and written back whenever a profile
//! changes over the API. Anything wrong with the file stops the server
//! rather than surfacing as a puzzling `400` later, so every check here
//! reports the profile and the field it is unhappy with. The same checks
//! give a request its `400`.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use starprint_workflows::{Head, Paper, Printer, PrinterKind, Speed, check_density};

/// A named profile and the printer it builds. The desktop app hands
/// its own profiles over in this shape, already checked by their type,
/// so only the file and the API go through the checks below.
#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub name: String,
    pub printer: Printer,
}

/// A profile as the file and the API spell it. `host`, `port`, `kind`
/// and `paper` are fixed; the rest are defaults a job request may
/// override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileSpec {
    pub host: String,
    pub port: u16,
    pub kind: PrinterKind,
    pub cut: bool,
    /// Thermal only, and required there: the roll width in millimetres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper: Option<u16>,
    /// Thermal only, and required there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub density: Option<i8>,
    /// Thermal only, and required there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<Speed>,
}

impl From<&Printer> for ProfileSpec {
    fn from(printer: &Printer) -> Self {
        let (paper, density, speed) = match printer.head {
            Head::Thermal {
                paper,
                density,
                speed,
            } => (Some(paper.mm()), Some(density), Some(speed)),
            Head::Impact => (None, None, None),
        };
        Self {
            host: printer.host.clone(),
            port: printer.port,
            kind: printer.head.kind(),
            cut: printer.cut,
            paper,
            density,
            speed,
        }
    }
}

impl Profile {
    /// Checks `spec` the way the file is checked, so a request gets the
    /// same answer a hand-written profile would.
    pub fn new(name: String, spec: ProfileSpec) -> Result<Self, String> {
        if name.trim().is_empty() {
            return Err("the profile's name is empty".to_owned());
        }
        if name.contains('/') {
            return Err(format!("{name}: a profile's name cannot contain `/`"));
        }
        if spec.host.trim().is_empty() {
            return Err(format!("{name}: `host` is empty"));
        }
        let mut printer = match spec.kind {
            PrinterKind::Thermal => {
                let mm = required(&name, "paper", spec.paper)?;
                let paper = Paper::from_mm(mm)
                    .ok_or_else(|| format!("{name}: `paper` is {mm}, not 80 or 112"))?;
                let density = required(&name, "density", spec.density)?;
                check_density(density).map_err(|e| format!("{name}: {e}"))?;
                let speed = required(&name, "speed", spec.speed)?;
                Printer::thermal(spec.host, spec.port, paper, density, speed)
            }
            PrinterKind::Impact => {
                for (field, present) in [
                    ("paper", spec.paper.is_some()),
                    ("density", spec.density.is_some()),
                    ("speed", spec.speed.is_some()),
                ] {
                    if present {
                        return Err(format!(
                            "{name}: `{field}` is not a setting on an impact printer"
                        ));
                    }
                }
                Printer::impact(spec.host, spec.port)
            }
        };
        printer.cut = spec.cut;
        Ok(Profile { name, printer })
    }

    pub fn spec(&self) -> ProfileSpec {
        ProfileSpec::from(&self.printer)
    }
}

/// Reads the file at `path`: a JSON object of profiles by name. The
/// profiles keep the order the file lists them in, so `GET
/// /v1/printers` reads like the file. No file is no printers.
pub fn read(path: &Path) -> Result<Vec<Profile>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes `profiles` to `path` in the order given.
pub fn write(path: &Path, profiles: &[Profile]) -> Result<(), String> {
    let file: Map<String, Value> = profiles
        .iter()
        .map(|profile| {
            let spec = serde_json::to_value(profile.spec()).expect("a profile is an object");
            (profile.name.clone(), spec)
        })
        .collect();
    let text = serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?;
    crate::data::replace(path, &text)
}

fn parse(text: &str) -> Result<Vec<Profile>, String> {
    let file: Map<String, Value> = serde_json::from_str(text).map_err(|e| e.to_string())?;
    file.into_iter()
        .map(|(name, value)| {
            let spec = ProfileSpec::deserialize(value).map_err(|e| format!("{name}: {e}"))?;
            Profile::new(name, spec)
        })
        .collect()
}

fn required<T>(name: &str, field: &str, value: Option<T>) -> Result<T, String> {
    value.ok_or_else(|| format!("{name}: `{field}` is required for a thermal printer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use starprint_workflows::Head;

    const SAMPLE: &str = r#"{
  "tsp800ii": { "host": "192.168.1.180", "port": 9100, "kind": "thermal", "paper": 80, "cut": true, "density": 3, "speed": "slow" },
  "sp743": { "host": "192.168.1.141", "port": 9100, "kind": "impact", "cut": true }
}"#;

    fn err(file: Value) -> String {
        parse(&file.to_string()).unwrap_err()
    }

    /// A valid thermal profile named `p`, with one field replaced.
    fn thermal_with(field: &str, value: Value) -> Value {
        let mut spec = json!({
            "host": "h", "port": 9100, "kind": "thermal", "cut": true,
            "paper": 80, "density": 0, "speed": "slow",
        });
        spec[field] = value;
        json!({ "p": spec })
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
        assert!(parse("{}").unwrap().is_empty());
    }

    #[test]
    fn a_missing_file_is_a_server_without_any() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(&dir.path().join("printers.json")).unwrap().is_empty());
    }

    /// What is written reads back as the same profiles, in the same
    /// order, with the thermal fields left off the impact printer.
    #[test]
    fn the_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("printers.json");
        let profiles = parse(SAMPLE).unwrap();
        write(&path, &profiles).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\n  \"tsp800ii\": {"), "{text}");
        assert!(!text.contains("\"paper\": null"), "{text}");

        let again = read(&path).unwrap();
        let specs = |profiles: &[Profile]| -> Vec<(String, ProfileSpec)> {
            profiles
                .iter()
                .map(|p| (p.name.clone(), p.spec()))
                .collect()
        };
        assert_eq!(specs(&again), specs(&profiles));
    }

    #[test]
    fn thermal_fields_are_required_on_a_thermal_printer() {
        let file = json!({ "p": {
            "host": "h", "port": 9100, "kind": "thermal", "cut": true, "density": 0, "speed": "slow",
        } });
        assert_eq!(err(file), "p: `paper` is required for a thermal printer");
    }

    #[test]
    fn thermal_fields_are_refused_on_an_impact_printer() {
        let file = json!({ "p": {
            "host": "h", "port": 9100, "kind": "impact", "cut": true, "density": 3,
        } });
        assert_eq!(
            err(file),
            "p: `density` is not a setting on an impact printer"
        );
    }

    #[test]
    fn invalid_values_are_reported_with_their_profile() {
        assert!(
            parse(&thermal_with("paper", json!(80)).to_string()).is_ok(),
            "the profile is valid"
        );
        assert_eq!(
            err(thermal_with("paper", json!(58))),
            "p: `paper` is 58, not 80 or 112"
        );
        assert_eq!(
            err(thermal_with("density", json!(9))),
            "p: `density` is 9, outside -3 to 4"
        );
        assert!(err(thermal_with("speed", json!("quick"))).starts_with("p: "));
        assert!(err(thermal_with("port", json!(70000))).starts_with("p: "));
    }

    #[test]
    fn an_unknown_field_stops_the_server() {
        let file = json!({ "p": {
            "host": "h", "port": 9100, "kind": "impact", "cut": true, "colour": true,
        } });
        assert!(err(file.clone()).contains("colour"), "{}", err(file));
    }

    #[test]
    fn a_file_that_is_not_an_object_stops_the_server() {
        assert!(parse("[]").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn an_empty_host_stops_the_server() {
        let file = json!({ "p": { "host": "  ", "port": 9100, "kind": "impact", "cut": true } });
        assert_eq!(err(file), "p: `host` is empty");
    }

    /// A name is a path segment on the API, so a slash cannot be part
    /// of one.
    #[test]
    fn a_name_that_cannot_be_addressed_is_refused() {
        let spec = ProfileSpec {
            host: "h".to_owned(),
            port: 9100,
            kind: PrinterKind::Impact,
            cut: true,
            paper: None,
            density: None,
            speed: None,
        };
        assert_eq!(
            Profile::new("  ".to_owned(), spec.clone()).unwrap_err(),
            "the profile's name is empty"
        );
        assert!(Profile::new("a/b".to_owned(), spec).is_err());
    }
}
