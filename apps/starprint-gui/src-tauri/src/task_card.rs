//! The task card: a bold, quad-size task with an optional "HIGH PRIORITY"
//! flag and a right-aligned due date above it. High priority prints red on
//! an SP700 and inverse on a thermal printer.
//!
//! The layout mirrors the sibling Python GUI (and the `task_card` example
//! in the library crate) byte for byte.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use starprint::{Builder, Color, Cut, Document, Impact, Protocol, StarLine};

/// What the user typed into the form.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCard {
    pub text: String,
    pub priority: bool,
    /// An ISO date (`2026-08-28`), which is printed as `28 AUG 2026`, or
    /// free text, which is printed as is.
    pub due: Option<String>,
}

/// A print-ready description of the card, so the UI can show exactly what
/// will come out of the printer: the same wrapping at the same widths.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// Characters per line at normal size.
    pub columns: usize,
    /// The priority banner text when the flag is set, else `None`.
    pub priority: Option<String>,
    /// The formatted due text, right-aligned to fill the header line.
    pub due: Option<String>,
    /// The task text, word-wrapped at quad size (half the columns).
    pub lines: Vec<String>,
}

/// The protocol-specific pieces of the card, so the card itself is built
/// once for both printers.
pub trait CardStyle: Sized {
    const COLUMNS: usize;
    /// The priority banner. Thermal prints it inverse, so it is padded
    /// with a space each side to give the black block some margin;
    /// impact prints plain red text, which wants no padding.
    const PRIORITY_TEXT: &'static str;

    fn set_wide(self, on: bool) -> Self;
    fn set_tall(self, on: bool) -> Self;
    fn set_accent(self, on: bool) -> Self;
}

impl CardStyle for Builder<StarLine> {
    const COLUMNS: usize = 48;
    const PRIORITY_TEXT: &'static str = " HIGH PRIORITY ";

    fn set_wide(self, on: bool) -> Self {
        self.wide(if on { 2 } else { 1 })
    }

    fn set_tall(self, on: bool) -> Self {
        self.tall(if on { 2 } else { 1 })
    }

    fn set_accent(self, on: bool) -> Self {
        self.invert(on)
    }
}

impl CardStyle for Builder<Impact> {
    const COLUMNS: usize = 42;
    const PRIORITY_TEXT: &'static str = "HIGH PRIORITY";

    fn set_wide(self, on: bool) -> Self {
        self.double_wide(on)
    }

    fn set_tall(self, on: bool) -> Self {
        self.double_tall(on)
    }

    fn set_accent(self, on: bool) -> Self {
        self.color(if on { Color::Red } else { Color::Black })
    }
}

fn wrap_by_words(text: &str, max_len: usize) -> Vec<String> {
    let raw_lines: Vec<&str> = if text.is_empty() {
        vec![""]
    } else {
        text.lines().collect()
    };
    let mut lines = Vec::new();

    for raw_line in raw_lines {
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        let mut current = String::new();
        for word in &words {
            let word_len = word.chars().count();
            if current.is_empty() {
                current.push_str(word);
            } else if current.chars().count() + 1 + word_len <= max_len {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current);
                current = (*word).to_owned();
            }
        }
        lines.push(current);
    }
    lines
}

fn format_date(date: NaiveDate) -> String {
    const MONTHS: [&str; 12] = [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    format!(
        "{:02} {} {:04}",
        date.day(),
        MONTHS[date.month0() as usize],
        date.year()
    )
}

fn due_text(due: Option<&str>) -> Option<String> {
    let raw = due.map(str::trim).filter(|value| !value.is_empty())?;
    Some(
        NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .map(format_date)
            .unwrap_or_else(|_| raw.to_owned()),
    )
}

fn align_right(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(text.chars().count());
    format!("{}{text}", " ".repeat(padding))
}

impl TaskCard {
    pub fn layout<P: Protocol>(&self) -> Layout
    where
        Builder<P>: CardStyle,
    {
        let columns = <Builder<P> as CardStyle>::COLUMNS;
        let priority = self
            .priority
            .then(|| <Builder<P> as CardStyle>::PRIORITY_TEXT.to_owned());
        let due = due_text(self.due.as_deref()).map(|due| {
            let width = columns.saturating_sub(priority.as_ref().map_or(0, |p| p.chars().count()));
            align_right(&due, width)
        });
        Layout {
            columns,
            priority,
            due,
            lines: wrap_by_words(self.text.trim(), columns / 2),
        }
    }

    /// Builds the print job. `cut` feeds and cuts after the card;
    /// without it the card only feeds clear of the head.
    pub fn document<P: Protocol>(&self, builder: Builder<P>, cut: bool) -> Document
    where
        Builder<P>: CardStyle,
    {
        let layout = self.layout::<P>();
        let mut card = builder.bold(true);
        if layout.priority.is_some() || layout.due.is_some() {
            if let Some(priority) = &layout.priority {
                card = card.set_accent(true).text(priority).set_accent(false);
            }
            if let Some(due) = &layout.due {
                card = card.bold(false).text(due).bold(true);
            }
            card = card.raw([b'\n']).feed(1);
        }

        let card = card
            .set_wide(true)
            .set_tall(true)
            .text(&layout.lines.join("\n"))
            .feed(2);
        if cut {
            card.cut(Cut::FeedThenPartial).build()
        } else {
            card.feed(3).build()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(hex: &str) -> Vec<u8> {
        hex.split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }

    fn card(text: &str, priority: bool, due: Option<&str>) -> TaskCard {
        TaskCard {
            text: text.to_owned(),
            priority,
            due: due.map(str::to_owned),
        }
    }

    #[test]
    fn basic_impact_card_matches_gui_fixture() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 57 01 1b 68 01 54 65 73 74 20 74 61 73 6b 1b 61 02 1b 64 03",
        );
        let actual = card("Test task", false, None).document(starprint::impact(), true);
        assert_eq!(actual.as_bytes(), expected);
    }

    #[test]
    fn card_without_cut_only_feeds() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 57 01 1b 68 01 54 65 73 74 20 74 61 73 6b 1b 61 02 1b 61 03",
        );
        let actual = card("Test task", false, None).document(starprint::impact(), false);
        assert_eq!(actual.as_bytes(), expected);
    }

    /// The Python GUI pads the banner with a space each side on every
    /// printer; on impact the banner is plain red text, so the padding
    /// is dropped and the due date gains two columns.
    #[test]
    fn impact_priority_banner_is_unpadded() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 34 48 49 47 48 20 50 52 49 4f 52 49 54 59 1b 35 1b 46 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 31 35 20 4a 41 4e 20 32 30 32 35 1b 45 0a 1b 61 01 1b 57 01 1b 68 01 55 72 67 65 6e 74 20 74 61 73 6b 1b 61 02 1b 64 03",
        );
        let actual =
            card("Urgent task", true, Some("2025-01-15")).document(starprint::impact(), true);
        assert_eq!(actual.as_bytes(), expected);
    }

    #[test]
    fn thermal_priority_banner_keeps_padding() {
        let layout = card("x", true, None).layout::<StarLine>();
        assert_eq!(layout.priority.as_deref(), Some(" HIGH PRIORITY "));
        let layout = card("x", true, None).layout::<Impact>();
        assert_eq!(layout.priority.as_deref(), Some("HIGH PRIORITY"));
    }

    #[test]
    fn due_accepts_iso_dates_and_free_text() {
        assert_eq!(due_text(Some("2025-02-03")).as_deref(), Some("03 FEB 2025"));
        assert_eq!(due_text(Some("someday")).as_deref(), Some("someday"));
        assert_eq!(due_text(Some("  ")), None);
        assert_eq!(due_text(None), None);
    }

    #[test]
    fn layout_wraps_at_quad_size_width() {
        assert_eq!(
            wrap_by_words("one two three four", 10),
            ["one two", "three four"]
        );
        let layout = card("one two", false, None).layout::<Impact>();
        assert_eq!(layout.columns, 42);
        assert_eq!(layout.lines, ["one two"]);
        assert_eq!(layout.priority, None);
        assert_eq!(layout.due, None);
    }

    #[test]
    fn layout_right_aligns_due_after_priority() {
        let layout = card("x", true, Some("2025-01-15")).layout::<StarLine>();
        assert_eq!(layout.columns, 48);
        assert_eq!(layout.priority.as_deref(), Some(" HIGH PRIORITY "));
        // 48 columns minus the 15-character banner leaves 33, so the
        // 11-character date gets 22 spaces of padding.
        assert_eq!(
            layout.due.as_deref(),
            Some("                      15 JAN 2025")
        );
    }
}
