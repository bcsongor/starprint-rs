//! A bold, quad-size task with an optional "HIGH PRIORITY" banner and a
//! due date above it. Byte-identical to the Python GUI's card.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use starprint::{Builder, Cut, Document, Impact, Protocol, StarLine};

use crate::Paper;
use crate::text::{TextStyle, wrap_by_words};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCard {
    pub text: String,
    pub priority: bool,
    /// An ISO date, printed as `28 AUG 2026`, or free text printed as is.
    pub due: Option<String>,
}

/// The card as it will print, for the preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub columns: usize,
    pub priority: Option<String>,
    /// Right-aligned to fill the header line.
    pub due: Option<String>,
    /// Wrapped at half the columns, for quad size.
    pub lines: Vec<String>,
}

/// Thermal prints the banner inverse, so it gets a space each side for
/// margin; impact prints it in red and wants none.
pub trait CardStyle: TextStyle {
    const PRIORITY_TEXT: &'static str;
}

impl CardStyle for Builder<StarLine> {
    const PRIORITY_TEXT: &'static str = " HIGH PRIORITY ";
}

impl CardStyle for Builder<Impact> {
    const PRIORITY_TEXT: &'static str = "HIGH PRIORITY";
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
    pub fn layout<P: Protocol>(&self, paper: Paper) -> Layout
    where
        Builder<P>: CardStyle,
    {
        let columns = <Builder<P> as TextStyle>::columns(paper);
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

    /// Without `cut` the card only feeds clear of the head.
    pub fn document<P: Protocol>(&self, builder: Builder<P>, paper: Paper, cut: bool) -> Document
    where
        Builder<P>: CardStyle,
    {
        let layout = self.layout::<P>(paper);
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
        let actual =
            card("Test task", false, None).document(starprint::impact(), Paper::Mm80, true);
        assert_eq!(actual.as_bytes(), expected);
    }

    #[test]
    fn card_without_cut_only_feeds() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 57 01 1b 68 01 54 65 73 74 20 74 61 73 6b 1b 61 02 1b 61 03",
        );
        let actual =
            card("Test task", false, None).document(starprint::impact(), Paper::Mm80, false);
        assert_eq!(actual.as_bytes(), expected);
    }

    /// Deliberately differs from the Python GUI, which pads on impact too.
    #[test]
    fn impact_priority_banner_is_unpadded() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 34 48 49 47 48 20 50 52 49 4f 52 49 54 59 1b 35 1b 46 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 31 35 20 4a 41 4e 20 32 30 32 35 1b 45 0a 1b 61 01 1b 57 01 1b 68 01 55 72 67 65 6e 74 20 74 61 73 6b 1b 61 02 1b 64 03",
        );
        let actual = card("Urgent task", true, Some("2025-01-15")).document(
            starprint::impact(),
            Paper::Mm80,
            true,
        );
        assert_eq!(actual.as_bytes(), expected);
    }

    #[test]
    fn thermal_priority_banner_keeps_padding() {
        let layout = card("x", true, None).layout::<StarLine>(Paper::Mm80);
        assert_eq!(layout.priority.as_deref(), Some(" HIGH PRIORITY "));
        let layout = card("x", true, None).layout::<Impact>(Paper::Mm80);
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
        let layout = card("one two", false, None).layout::<Impact>(Paper::Mm112);
        assert_eq!(layout.columns, 42, "impact ignores the roll");
        assert_eq!(layout.lines, ["one two"]);
        assert_eq!(layout.priority, None);
        assert_eq!(layout.due, None);
    }

    #[test]
    fn layout_right_aligns_due_after_priority() {
        let layout = card("x", true, Some("2025-01-15")).layout::<StarLine>(Paper::Mm80);
        assert_eq!(layout.columns, 48);
        assert_eq!(layout.priority.as_deref(), Some(" HIGH PRIORITY "));
        // 48 - 15 banner = 33; 33 - 11 date = 22 spaces.
        assert_eq!(
            layout.due.as_deref(),
            Some("                      15 JAN 2025")
        );
    }

    #[test]
    fn wider_paper_gives_the_card_more_columns() {
        let layout = card("x", false, None).layout::<StarLine>(Paper::Mm112);
        assert_eq!(layout.columns, 69, "832 dots of Font A");
    }
}
