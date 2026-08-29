//! Plain text in the styles the head offers, wrapped to the paper. The
//! Python GUI's `simple_text` workflow.

use serde::{Deserialize, Serialize};
use starprint::{Builder, Color, Cut, Document, Impact, Protocol, StarLine};

use crate::Paper;

/// What differs between the two builders when laying out characters.
pub trait TextStyle: Sized {
    /// At normal size.
    fn columns(paper: Paper) -> usize;

    fn set_wide(self, on: bool) -> Self;
    fn set_tall(self, on: bool) -> Self;
    /// Red on impact; thermal has no second colour, so inverse.
    fn set_accent(self, on: bool) -> Self;
}

impl TextStyle for Builder<StarLine> {
    fn columns(paper: Paper) -> usize {
        paper.columns()
    }

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

impl TextStyle for Builder<Impact> {
    /// The SP700's carriage is fixed.
    fn columns(_paper: Paper) -> usize {
        42
    }

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

/// Keeps the line breaks the user typed.
pub fn wrap_by_words(text: &str, max_len: usize) -> Vec<String> {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub text: String,
    pub bold: bool,
    pub wide: bool,
    pub tall: bool,
    /// Red on impact, inverse on thermal.
    pub accent: bool,
}

/// The text as it will print, for the preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// At normal size, whatever the chosen width.
    pub columns: usize,
    pub lines: Vec<String>,
}

impl Text {
    pub fn layout<P: Protocol>(&self, paper: Paper) -> Layout
    where
        Builder<P>: TextStyle,
    {
        let columns = <Builder<P> as TextStyle>::columns(paper);
        Layout {
            columns,
            lines: wrap_by_words(
                self.text.trim(),
                if self.wide { columns / 2 } else { columns },
            ),
        }
    }

    /// Without `cut` nothing feeds the paper, so the next job starts on
    /// the line under this one.
    pub fn document<P: Protocol>(&self, builder: Builder<P>, paper: Paper, cut: bool) -> Document
    where
        Builder<P>: TextStyle,
    {
        let layout = self.layout::<P>(paper);
        let mut doc = builder;
        if self.bold {
            doc = doc.bold(true);
        }
        if self.wide {
            doc = doc.set_wide(true);
        }
        if self.tall {
            doc = doc.set_tall(true);
        }
        if self.accent {
            doc = doc.set_accent(true);
        }

        doc = doc.text(&layout.lines.join("\n"));

        // Nothing carries over to the next job.
        if self.accent {
            doc = doc.set_accent(false);
        }
        if self.tall {
            doc = doc.set_tall(false);
        }
        if self.wide {
            doc = doc.set_wide(false);
        }
        if self.bold {
            doc = doc.bold(false);
        }

        // After the styles are put back, so the feed is one normal line.
        // The cut feeds to the cutter itself; anything more is blank paper.
        let doc = doc.raw([b'\n']);
        if cut {
            doc.cut(Cut::FeedThenPartial).build()
        } else {
            doc.build()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> Text {
        Text {
            text: text.to_owned(),
            bold: false,
            wide: false,
            tall: false,
            accent: false,
        }
    }

    #[test]
    fn wrapping_follows_the_paper_and_the_width() {
        // The first ten words are exactly 48 characters.
        let text = plain("one two three four five six seven eight nine ten eleven twelve");
        let narrow = text.layout::<StarLine>(Paper::Mm80);
        assert_eq!(narrow.columns, 48);
        assert_eq!(
            narrow.lines,
            [
                "one two three four five six seven eight nine ten",
                "eleven twelve"
            ]
        );

        let wide = text.layout::<StarLine>(Paper::Mm112);
        assert_eq!(wide.columns, 69, "832 dots of Font A");
        assert_eq!(wide.lines.len(), 1);
    }

    #[test]
    fn double_width_halves_the_line() {
        let mut text = plain("one two three four five six seven eight nine ten eleven twelve");
        text.wide = true;
        let layout = text.layout::<StarLine>(Paper::Mm80);
        assert_eq!(layout.columns, 48, "still the paper's columns");
        assert!(
            layout.lines.iter().all(|line| line.chars().count() <= 24),
            "wrapped at 24: {:?}",
            layout.lines
        );
    }

    #[test]
    fn the_impact_head_ignores_the_roll() {
        let layout = plain("x").layout::<Impact>(Paper::Mm112);
        assert_eq!(layout.columns, 42);
    }

    #[test]
    fn styles_are_set_and_put_back_around_the_text() {
        let text = Text {
            text: "Hi".to_owned(),
            bold: true,
            wide: true,
            tall: true,
            accent: true,
        };
        let bytes = text
            .document(starprint::impact(), Paper::Mm80, true)
            .as_bytes()
            .to_vec();
        // ESC E bold on, ESC W 1 wide, ESC h 1 tall, ESC 4 red, "Hi",
        // then ESC 5, ESC h 0, ESC W 0, ESC F.
        let expected = [
            0x1b, b'E', 0x1b, b'W', 1, 0x1b, b'h', 1, 0x1b, b'4', b'H', b'i', 0x1b, b'5', 0x1b,
            b'h', 0, 0x1b, b'W', 0, 0x1b, b'F',
        ];
        assert!(
            bytes.windows(expected.len()).any(|w| w == expected),
            "styles bracket the text: {bytes:02x?}"
        );
        assert!(
            bytes.ends_with(&[b'\n', 0x1b, b'd', 3]),
            "a line feed, then the cut, and no blank paper between"
        );
    }

    #[test]
    fn plain_text_carries_no_style_commands() {
        let bytes = plain("Hi")
            .document(starprint::starline(), Paper::Mm80, false)
            .as_bytes()
            .to_vec();
        assert!(bytes.windows(2).any(|w| w == [b'H', b'i']));
        assert!(!bytes.windows(2).any(|w| w == [0x1b, b'E']), "no bold");
        assert!(
            bytes.ends_with(b"Hi\n"),
            "without a cut the job ends on the line feed: {bytes:02x?}"
        );
    }

    #[test]
    fn jobs_without_a_cut_run_into_the_next() {
        let first = plain("hello").document(starprint::starline(), Paper::Mm80, false);
        assert!(
            first.as_bytes().ends_with(b"hello\n"),
            "{:02x?}",
            first.as_bytes()
        );
        assert!(
            !first.as_bytes().windows(2).any(|w| w == [0x1b, b'a']),
            "no feed command at all"
        );

        let mut second = plain("world");
        second.bold = true;
        let bytes = second.document(starprint::starline(), Paper::Mm80, true);
        let bytes = bytes.as_bytes();
        assert!(bytes.windows(5).any(|w| w == b"world"));
        assert!(bytes.ends_with(&[b'\n', 0x1b, b'd', 3]));
    }

    #[test]
    fn typed_line_breaks_are_kept() {
        assert_eq!(wrap_by_words("one\ntwo", 40), ["one", "two"]);
        assert_eq!(wrap_by_words("", 40), [""]);
    }
}
