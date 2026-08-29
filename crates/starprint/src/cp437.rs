//! CP437 encoding. Star printers read each text byte through the selected
//! code page, so strings are encoded rather than sent as UTF-8. Unmappable
//! characters become `?` so layouts keep their alignment.

/// 0x80..=0xFF of CP437; the lower half is ASCII.
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

/// `None` if the character has no CP437 mapping. `\n` passes through as
/// the line feed.
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
