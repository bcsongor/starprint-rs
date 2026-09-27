//! The header a received fax prints under: a bar across the paper with
//! `FAX` and when it was sent, inverse on thermal and red on impact, then
//! who sent it.

use chrono::NaiveDateTime;
use starprint::{Builder, Protocol};

use crate::Paper;
use crate::task_card::format_date;
use crate::text::TextStyle;

/// Who sent a fax and when. The number was checked and prints bold; the
/// name is the sender's own and unchecked, so it prints plain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaxHeader {
    pub name: String,
    /// As written, like `*7441 720938`.
    pub number: String,
    /// On the recipient's clock.
    pub sent: NaiveDateTime,
}

/// The header as it will print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// `FAX` and the time, padded to the full width.
    pub bar: String,
    pub number: String,
    /// Padded on the left to end at the right edge.
    pub name: String,
}

impl FaxHeader {
    pub fn layout<P: Protocol>(&self, paper: Paper) -> Layout
    where
        Builder<P>: TextStyle,
    {
        let columns = <Builder<P> as TextStyle>::columns(paper);
        let sent = format!(
            "{} {}",
            format_date(self.sent.date()),
            self.sent.format("%H:%M")
        );
        // A space each end, so inverse text does not touch the edge.
        let bar = format!(" FAX{sent:>width$} ", width = columns.saturating_sub(5));

        // A control character in a name would move the head.
        let name: String = self
            .name
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let width = columns.saturating_sub(self.number.chars().count());
        let name: String = name.trim().chars().take(width.saturating_sub(2)).collect();
        Layout {
            bar,
            number: self.number.clone(),
            name: format!("{name:>width$}"),
        }
    }

    /// Prints the header on `builder`, leaving every style as it found
    /// it, so the fax that follows prints as it would on its own.
    pub fn print<P: Protocol>(&self, builder: Builder<P>, paper: Paper) -> Builder<P>
    where
        Builder<P>: TextStyle,
    {
        let layout = self.layout::<P>(paper);
        builder
            .bold(true)
            .set_accent(true)
            .text(&layout.bar)
            .set_accent(false)
            .raw(b"\n")
            .text(&layout.number)
            .bold(false)
            .text(&layout.name)
            .raw(b"\n")
            .feed(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use starprint::{Impact, StarLine};

    fn header(name: &str) -> FaxHeader {
        FaxHeader {
            name: name.to_owned(),
            number: "*2053 393035".to_owned(),
            sent: NaiveDate::from_ymd_opt(2026, 9, 27)
                .unwrap()
                .and_hms_opt(14, 32, 0)
                .unwrap(),
        }
    }

    #[test]
    fn the_header_fills_the_width() {
        for (layout, columns) in [
            (header("Anna").layout::<StarLine>(Paper::Mm80), 48),
            (header("Anna").layout::<StarLine>(Paper::Mm112), 69),
            (header("Anna").layout::<Impact>(Paper::Mm80), 42),
        ] {
            assert_eq!(layout.bar.chars().count(), columns);
            assert!(layout.bar.starts_with(" FAX  "));
            assert!(layout.bar.ends_with("  27 SEP 2026 14:32 "));
            let from = layout.number + &layout.name;
            assert_eq!(from.chars().count(), columns);
            assert!(from.starts_with("*2053 393035  ") && from.ends_with(" Anna"));
        }
    }

    #[test]
    fn a_long_or_unruly_name_stays_on_its_line() {
        let layout = header(&format!("Anna\n{}", "B".repeat(60))).layout::<StarLine>(Paper::Mm80);
        let from = layout.number + &layout.name;
        assert_eq!(from.chars().count(), 48);
        assert!(from.starts_with("*2053 393035  Anna B"), "{from}");
        let nameless = header("").layout::<StarLine>(Paper::Mm80);
        assert_eq!(nameless.name.trim(), "");
    }

    #[test]
    fn the_bar_is_inverse_on_thermal_and_red_on_impact() {
        let thermal = header("Anna")
            .print(starprint::starline(), Paper::Mm80)
            .build();
        let bytes = thermal.as_bytes();
        let on = bytes.windows(2).position(|w| w == [0x1b, b'4']);
        let off = bytes.windows(2).position(|w| w == [0x1b, b'5']);
        assert!(on.is_some() && on < off, "on, then off: {bytes:02x?}");

        let impact = header("Anna")
            .print(starprint::impact(), Paper::Mm80)
            .build();
        let bytes = impact.as_bytes();
        assert!(
            bytes.windows(2).any(|w| w == [0x1b, b'4']),
            "red: {bytes:02x?}"
        );
    }
}
