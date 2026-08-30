//! A job's bytes as text, so you can read its escape sequences without
//! a printer.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HexDump {
    pub bytes: usize,
    /// 16 bytes per row: offset, hex, ASCII.
    pub dump: String,
}

pub fn of(bytes: &[u8]) -> HexDump {
    let dump = bytes
        .chunks(16)
        .enumerate()
        .map(|(row, chunk)| {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            let ascii: String = chunk
                .iter()
                .map(|&b| {
                    if (0x20..0x7f).contains(&b) {
                        b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            format!("{:04x}  {:<47}  {ascii}", row * 16, hex.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n");
    HexDump {
        bytes: bytes.len(),
        dump,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_is_an_offset_then_hex_then_ascii() {
        let dump = of(b"\x1b@Hi");
        assert_eq!(dump.bytes, 4);
        let (hex, rest) = dump.dump.split_once("  .@Hi").expect("ends in ASCII");
        assert_eq!(hex.trim_end(), "0000  1b 40 48 69");
        assert!(rest.is_empty());
        // A full row of 16 bytes is 47 characters of hex, which is what
        // the short row above is padded to.
        assert_eq!(dump.dump.len(), 4 + 2 + 47 + 2 + 4, "the columns line up");
    }

    #[test]
    fn rows_hold_sixteen_bytes_each() {
        let dump = of(&[0u8; 33]);
        let offsets: Vec<&str> = dump.dump.lines().map(|l| &l[..4]).collect();
        assert_eq!(offsets, ["0000", "0010", "0020"]);
    }

    #[test]
    fn no_bytes_dump_to_nothing() {
        let dump = of(b"");
        assert_eq!(dump.bytes, 0);
        assert_eq!(dump.dump, "");
    }
}
