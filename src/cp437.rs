//! Text encoding for code page 437 (the factory-default single-byte code
//! page on Star receipt printers).
//!
//! Star printers do not understand UTF-8: each byte sent while printing text
//! is looked up in the currently selected code page. This module converts
//! Rust strings to CP437 so that common Western-European characters and the
//! classic box-drawing set print correctly out of the box.
//!
//! Characters outside CP437 are substituted with `?` rather than dropped, so
//! receipt layouts keep their alignment. Callers who select a different code
//! page on the printer should pre-encode their text and use the raw-bytes
//! escape hatch instead.

/// The upper half (0x80..=0xFF) of code page 437.
///
/// The lower half is identical to ASCII and is passed through untouched.
#[rustfmt::skip]
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', // 0x80
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', // 0x90
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', // 0xA0
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', // 0xB0
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', // 0xC0
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', // 0xD0
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', // 0xE0
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{A0}', // 0xF0
];

/// Encodes a single character as CP437, or `None` if it has no mapping.
///
/// Printable ASCII maps to itself; `\n` maps to the line-feed control byte
/// (`0x0A`), which prints and feeds the current line on Star printers.
pub(crate) fn encode_char(c: char) -> Option<u8> {
    match c {
        '\n' => Some(b'\n'),
        ' '..='~' => Some(c as u8),
        _ => CP437_HIGH
            .iter()
            .position(|&hi| hi == c)
            .map(|i| 0x80 + i as u8),
    }
}

/// Encodes a string as CP437, substituting `?` for unmappable characters,
/// and appends it to `out`.
pub(crate) fn encode_into(text: &str, out: &mut Vec<u8>) {
    out.extend(text.chars().map(|c| encode_char(c).unwrap_or(b'?')));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_passes_through() {
        let mut buf = Vec::new();
        encode_into("Receipt #42 (TOTAL: $7.00)\n", &mut buf);
        assert_eq!(buf, b"Receipt #42 (TOTAL: $7.00)\n");
    }

    #[test]
    fn high_half_maps_to_cp437() {
        let mut buf = Vec::new();
        encode_into("Café £9 ½", &mut buf);
        assert_eq!(buf, [b'C', b'a', b'f', 0x82, b' ', 0x9C, b'9', b' ', 0xAB]);
    }

    #[test]
    fn unmappable_becomes_question_mark() {
        let mut buf = Vec::new();
        encode_into("こんにちは €", &mut buf);
        assert_eq!(buf, b"????? ?");
    }

    #[test]
    fn table_round_trips() {
        for (i, &c) in CP437_HIGH.iter().enumerate() {
            assert_eq!(encode_char(c), Some(0x80 + i as u8), "char {c:?}");
        }
    }
}
