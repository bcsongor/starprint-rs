//! The task card: a bold, quad-size task with an optional "HIGH PRIORITY"
//! flag and a right-aligned due date above it. High priority prints red on
//! an SP700 and inverse on a thermal printer.
//!
//! The layout mirrors the sibling Python GUI (and the `task_card` example
//! in the library crate) byte for byte.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use starprint::{Builder, Color, Cut, Document, Impact, Protocol, StarLine};

const PRIORITY_TEXT: &str = " HIGH PRIORITY ";

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
    /// `PRIORITY_TEXT` when the priority flag is set, else `None`.
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

    fn set_wide(self, on: bool) -> Self;
    fn set_tall(self, on: bool) -> Self;
    fn set_accent(self, on: bool) -> Self;
}

impl CardStyle for Builder<StarLine> {
    const COLUMNS: usize = 48;

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
    pub fn layout(&self, columns: usize) -> Layout {
        let priority = self.priority.then(|| PRIORITY_TEXT.to_owned());
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

    pub fn document<P: Protocol>(&self, builder: Builder<P>) -> Document
    where
        Builder<P>: CardStyle,
    {
        let layout = self.layout(<Builder<P> as CardStyle>::COLUMNS);
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

        card.set_wide(true)
            .set_tall(true)
            .text(&layout.lines.join("\n"))
            .feed(2)
            .cut(Cut::FeedThenPartial)
            .build()
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
        let actual = card("Test task", false, None).document(starprint::impact());
        assert_eq!(actual.as_bytes(), expected);
    }

    #[test]
    fn priority_card_emphasises_header_and_separates_task() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 34 20 48 49 47 48 20 50 52 49 4f 52 49 54 59 20 1b 35 1b 46 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 31 35 20 4a 41 4e 20 32 30 32 35 1b 45 0a 1b 61 01 1b 57 01 1b 68 01 55 72 67 65 6e 74 20 74 61 73 6b 1b 61 02 1b 64 03",
        );
        let actual = card("Urgent task", true, Some("2025-01-15")).document(starprint::impact());
        assert_eq!(actual.as_bytes(), expected);
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
        let layout = card("one two three four", false, None).layout(20);
        assert_eq!(layout.lines, ["one two", "three four"]);
        assert_eq!(layout.priority, None);
        assert_eq!(layout.due, None);
    }

    #[test]
    fn layout_right_aligns_due_after_priority() {
        let layout = card("x", true, Some("2025-01-15")).layout(42);
        assert_eq!(layout.priority.as_deref(), Some(PRIORITY_TEXT));
        // 42 columns minus the 15-character priority flag leaves 27, so
        // the 11-character date gets 16 spaces of padding.
        assert_eq!(layout.due.as_deref(), Some("                15 JAN 2025"));
    }
}
