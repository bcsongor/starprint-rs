//! The header a received fax prints under: a bar across the paper with
//! `FAX` and when it was sent, inverse on thermal and red on impact, then
//! who sent it.

use chrono::NaiveDateTime;
use starprint::{Builder, Protocol};

use crate::Paper;
use crate::task_card::format_date;
use crate::text::TextStyle;

/// Who sent a fax and when. The number was checked and prints bold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaxHeader {
    /// What the recipient's fax book calls the sender, or empty.
    pub name: String,
    /// As written, like `*star1en2su3z68yscvky0n3j3l2qwny4dkq7s`.
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
    /// Beside the number when there is room, padded on the left to end
    /// at the right edge. On a line of its own under the number when
    /// not, starting where the number does.
    pub name: String,
    pub name_below: bool,
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
        // An inverse bar has edges, which its text keeps a space from.
        // Red text has none, and lines up with the number under it.
        let pad = if <Builder<P> as TextStyle>::ACCENT_FILLS {
            " "
        } else {
            ""
        };
        let width = columns.saturating_sub(3 + 2 * pad.len());
        let bar = format!("{pad}FAX{sent:>width$}{pad}");

        // A control character in a name would move the head.
        let name: String = self
            .name
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let name = name.trim();
        let beside = columns.saturating_sub(self.number.chars().count());
        let name_below = name.chars().count() + 2 > beside;
        let width = if name_below { columns } else { beside };
        let name: String = name.chars().take(width).collect();
        Layout {
            bar,
            number: self.number.clone(),
            name: if name_below {
                name
            } else {
                format!("{name:>width$}")
            },
            name_below,
        }
    }

    /// Prints the header on `builder`, leaving every style as it found
    /// it, so the fax that follows prints as it would on its own.
    pub fn print<P: Protocol>(&self, builder: Builder<P>, paper: Paper) -> Builder<P>
    where
        Builder<P>: TextStyle,
    {
        let layout = self.layout::<P>(paper);
        let builder = builder
            .bold(true)
            .set_accent(true)
            .text(&layout.bar)
            .set_accent(false)
            .raw(b"\n")
            .text(&layout.number)
            .bold(false);
        let builder = if layout.name_below {
            builder.raw(b"\n")
        } else {
            builder
        };
        builder.text(&layout.name).raw(b"\n").feed(1)
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
            number: "*star15089lwj8gn70m8gepwymguzl5qkangla".to_owned(),
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
        ] {
            assert_eq!(layout.bar.chars().count(), columns);
            assert!(layout.bar.starts_with(" FAX  "));
            assert!(layout.bar.ends_with("  27 SEP 2026 14:32 "));
            assert!(!layout.name_below);
            let from = layout.number + &layout.name;
            assert_eq!(from.chars().count(), columns);
            assert!(
                from.starts_with("*star15089lwj8gn70m8gepwymguzl5qkangla  ")
                    && from.ends_with(" Anna")
            );
        }
        // Red text has no bar to stand clear of, so it reaches both edges.
        let impact = header("Anna").layout::<Impact>(Paper::Mm80);
        assert_eq!(impact.bar.chars().count(), 42);
        assert!(impact.bar.starts_with("FAX  ") && impact.bar.ends_with("  27 SEP 2026 14:32"));
    }

    #[test]
    fn a_name_without_room_goes_under_the_number() {
        let layout = header(&format!("Anna\n{}", "B".repeat(60))).layout::<StarLine>(Paper::Mm80);
        assert!(layout.name_below);
        assert_eq!(layout.name.chars().count(), 48);
        assert!(layout.name.starts_with("Anna B"), "{}", layout.name);

        // A number leaves a 42-column impact line no room for a name.
        let layout = header("Anna").layout::<Impact>(Paper::Mm80);
        assert!(layout.name_below);
        assert_eq!(layout.name, "Anna");
        let bytes = header("Anna")
            .print(starprint::impact(), Paper::Mm80)
            .build();
        let text = String::from_utf8_lossy(bytes.as_bytes()).into_owned();
        let (number, name) = (text.find("qkangla").unwrap(), text.find("Anna").unwrap());
        assert!(text[number..name].contains('\n'), "{text:?}");

        let nameless = header("").layout::<StarLine>(Paper::Mm80);
        assert!(!nameless.name_below);
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
