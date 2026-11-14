//! Plain text in the styles the head offers, wrapped to the paper.

use std::mem;
use std::ops::Not;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use starprint::graphics::DeviceProfile;
use starprint::{Builder, Color, Cut, Document, Impact, Protocol, StarLine};

use crate::Paper;

/// The SP700's carriage is fixed, so its 7×9 font always sets this many
/// characters to a line.
pub(crate) const IMPACT_COLUMNS: usize = 42;

/// What differs between the two builders: the head's geometry and the
/// styles its characters take.
pub(crate) trait TextStyle: Sized {
    /// At normal size.
    fn columns(paper: Paper) -> usize;

    /// The head and roll as geometry, for whatever a job draws as dots.
    fn profile(paper: Paper) -> &'static DeviceProfile;

    fn set_wide(self, on: bool) -> Self;
    fn set_tall(self, on: bool) -> Self;
    /// Red on impact; thermal has no second colour, so inverse.
    fn set_accent(self, on: bool) -> Self;
    /// Whether the accent inks the cell around a character, as inverse
    /// does, so that a line of it is a bar with edges of its own.
    const ACCENT_FILLS: bool;
}

impl TextStyle for Builder<StarLine> {
    fn columns(paper: Paper) -> usize {
        paper.columns()
    }

    fn profile(paper: Paper) -> &'static DeviceProfile {
        paper.profile()
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

    const ACCENT_FILLS: bool = true;
}

impl TextStyle for Builder<Impact> {
    fn columns(_paper: Paper) -> usize {
        IMPACT_COLUMNS
    }

    fn profile(_paper: Paper) -> &'static DeviceProfile {
        &DeviceProfile::SP700
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

    const ACCENT_FILLS: bool = false;
}

/// The styles a character prints in. Every one is off by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    bold: bool,
    wide: bool,
    tall: bool,
    accent: bool,
}

impl Style {
    /// How many columns of the paper a character takes.
    fn width(self) -> usize {
        if self.wide { 2 } else { 1 }
    }

    /// Switches `doc` from `self` to `to`. What goes off goes first, in
    /// the reverse of the order things come on, so the styles nest.
    fn switch<P: Protocol>(self, to: Self, mut doc: Builder<P>) -> Builder<P>
    where
        Builder<P>: TextStyle,
    {
        if self.accent && !to.accent {
            doc = doc.set_accent(false);
        }
        if self.tall && !to.tall {
            doc = doc.set_tall(false);
        }
        if self.wide && !to.wide {
            doc = doc.set_wide(false);
        }
        if self.bold && !to.bold {
            doc = doc.bold(false);
        }
        if to.bold && !self.bold {
            doc = doc.bold(true);
        }
        if to.wide && !self.wide {
            doc = doc.set_wide(true);
        }
        if to.tall && !self.tall {
            doc = doc.set_tall(true);
        }
        if to.accent && !self.accent {
            doc = doc.set_accent(true);
        }
        doc
    }
}

/// A character and the style it prints in.
type Cell = (char, Style);

/// Keeps the line breaks and the spaces the user typed, so text can be
/// lined up by hand and blank lines feed paper. A gap that would run its
/// word past the edge is dropped, and so are trailing spaces.
pub(crate) fn wrap_by_words(text: &str, max_len: usize) -> Vec<String> {
    wrap(&[Run::plain(text)], max_len)
        .into_iter()
        .map(|line| line.into_iter().map(|run| run.text).collect())
        .collect()
}

/// [`wrap_by_words`] for styled text, where a wide character takes two
/// of the `columns`. Each line is its runs, merged where the style does
/// not change.
///
/// A blank line is one empty run in the style of the break that ends
/// it, or of the line before when nothing does. That is the style its
/// line feed prints in, so a blank line in tall text feeds a tall line,
/// and text in one style prints the bytes it did before runs.
fn wrap(runs: &[Run], columns: usize) -> Vec<Vec<Run>> {
    let cells: Vec<Cell> = runs
        .iter()
        .flat_map(|run| run.text.chars().map(move |c| (c, run.style())))
        .collect();
    let breaks: Vec<Style> = cells
        .iter()
        .filter(|&&(c, _)| c == '\n')
        .map(|&(_, style)| style)
        .collect();
    let mut style = Style::default();
    let mut lines = Vec::new();
    // Not `lines()`, which would swallow a final blank line.
    for (index, raw) in cells.split(|&(c, _)| c == '\n').enumerate() {
        for line in wrap_line(raw, columns) {
            lines.push(match line.last() {
                Some(&(_, last)) => {
                    style = last;
                    join(&line)
                }
                None => {
                    style = breaks.get(index).copied().unwrap_or(style);
                    vec![Run::new(String::new(), style)]
                }
            });
        }
    }
    lines
}

fn width(cells: &[Cell]) -> usize {
    cells.iter().map(|(_, style)| style.width()).sum()
}

fn wrap_line(line: &[Cell], columns: usize) -> Vec<Vec<Cell>> {
    let end = line
        .iter()
        .rposition(|(c, _)| !c.is_whitespace())
        .map_or(0, |last| last + 1);
    let mut rest = &line[..end];
    let mut lines = Vec::new();
    let mut current = Vec::new();
    // Trailing whitespace is gone, so every gap has a word after it.
    while !rest.is_empty() {
        let (gap, after) = rest.split_at(
            rest.iter()
                .position(|(c, _)| !c.is_whitespace())
                .unwrap_or(rest.len()),
        );
        let (word, after) = after.split_at(
            after
                .iter()
                .position(|(c, _)| c.is_whitespace())
                .unwrap_or(after.len()),
        );
        rest = after;
        if width(&current) + width(gap) + width(word) <= columns {
            // A tab has no CP437 character, so it prints as a space.
            current.extend(gap.iter().map(|&(_, style)| (' ', style)));
        } else if !current.is_empty() {
            lines.push(mem::take(&mut current));
        }
        current.extend_from_slice(word);
    }
    lines.push(current);
    lines
}

/// `cells` as runs, one for each change of style.
fn join(cells: &[Cell]) -> Vec<Run> {
    cells
        .chunk_by(|(_, a), (_, b)| a == b)
        .map(|chunk| Run::new(chunk.iter().map(|&(c, _)| c).collect(), chunk[0].1))
        .collect()
}

/// A stretch of text in one style.
// A style that is off is left out of the JSON, which keeps a fax of
// many runs inside its limit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// Wraps to the paper, and keeps typed spaces and blank lines.
    pub text: String,
    #[serde(default, skip_serializing_if = "Not::not")]
    pub bold: bool,
    /// Double width.
    #[serde(default, skip_serializing_if = "Not::not")]
    pub wide: bool,
    /// Double height.
    #[serde(default, skip_serializing_if = "Not::not")]
    pub tall: bool,
    /// Red on impact, inverse on thermal.
    #[serde(default, skip_serializing_if = "Not::not")]
    pub accent: bool,
}

impl Run {
    /// `text` with every style off.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    fn new(text: String, style: Style) -> Self {
        let Style {
            bold,
            wide,
            tall,
            accent,
        } = style;
        Self {
            text,
            bold,
            wide,
            tall,
            accent,
        }
    }

    fn style(&self) -> Style {
        Style {
            bold: self.bold,
            wide: self.wide,
            tall: self.tall,
            accent: self.accent,
        }
    }
}

/// Text in one style, or runs for text whose style changes partway.
// Read in either shape of `Wire` and written as the plain one when one
// run will do, so that servers from before runs, a fax's recipient
// among them, read it as they always have. Text whose style changes
// partway still prints there, without its styles.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(from = "Wire", into = "Wire")]
#[schemars(inline)]
pub struct Text {
    pub runs: Vec<Run>,
}

/// The two shapes a text job's JSON takes.
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
enum Wire {
    /// Text whose style changes partway, as runs in order, each in its
    /// own style. A line break is a `\n` inside a run.
    Runs {
        runs: Vec<Run>,
        // The runs' characters, written for servers from before runs,
        // which require `text` and ignore `runs`. Nothing here reads it.
        #[serde(default, skip_deserializing)]
        #[schemars(skip)]
        text: String,
    },
    /// The whole text in one style.
    Plain(Run),
}

impl From<Wire> for Text {
    fn from(wire: Wire) -> Self {
        match wire {
            Wire::Runs { runs, .. } => Self { runs },
            Wire::Plain(run) => Self { runs: vec![run] },
        }
    }
}

impl From<Text> for Wire {
    fn from(text: Text) -> Self {
        let mut runs = text.runs;
        match runs.len() {
            0 => Self::Plain(Run::default()),
            1 => Self::Plain(runs.remove(0)),
            _ => Self::Runs {
                text: runs.iter().map(|run| run.text.as_str()).collect(),
                runs,
            },
        }
    }
}

/// The text as it will print, for the preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// At normal size, so a wide character takes two.
    pub columns: usize,
    /// Each line's characters, whatever their styles, as clients from
    /// before runs read a layout.
    pub lines: Vec<String>,
    /// Each line as its runs. A blank line is one empty run, in the
    /// style its line feed prints in.
    pub runs: Vec<Vec<Run>>,
}

impl Text {
    /// `text` in one style, with every style off.
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            runs: vec![Run::plain(text)],
        }
    }

    /// Whether nothing would print but blank paper.
    pub fn is_blank(&self) -> bool {
        self.runs.iter().all(|run| run.text.trim().is_empty())
    }

    pub(crate) fn layout<P: Protocol>(&self, paper: Paper) -> Layout
    where
        Builder<P>: TextStyle,
    {
        let columns = <Builder<P> as TextStyle>::columns(paper);
        let runs = wrap(&self.runs, columns);
        Layout {
            columns,
            lines: runs
                .iter()
                .map(|line| line.iter().map(|run| run.text.as_str()).collect())
                .collect(),
            runs,
        }
    }

    /// Without `cut` nothing feeds the paper, so the next job starts on
    /// the line under this one.
    pub(crate) fn document<P: Protocol>(
        &self,
        builder: Builder<P>,
        paper: Paper,
        cut: bool,
    ) -> Document
    where
        Builder<P>: TextStyle,
    {
        let mut doc = builder;
        let mut style = Style::default();
        for (index, line) in self.layout::<P>(paper).runs.iter().enumerate() {
            // In the style the last line ended in, as a job in one style
            // has always fed its lines.
            if index > 0 {
                doc = doc.raw(b"\n");
            }
            for run in line {
                doc = style.switch(run.style(), doc).text(&run.text);
                style = run.style();
            }
        }

        // Nothing carries over to the next job, and the feed after the
        // text is one normal line. The cut feeds to the cutter itself;
        // anything more is blank paper.
        let doc = style.switch(Style::default(), doc).raw(b"\n");
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
        Text::plain(text)
    }

    fn styled(text: &str, style: Style) -> Run {
        Run::new(text.to_owned(), style)
    }

    const WIDE: Style = Style {
        bold: false,
        wide: true,
        tall: false,
        accent: false,
    };

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
        text.runs[0].wide = true;
        let layout = text.layout::<StarLine>(Paper::Mm80);
        assert_eq!(layout.columns, 48, "still the paper's columns");
        let lines = layout.lines;
        assert!(
            lines.iter().all(|line| line.chars().count() <= 24),
            "wrapped at 24: {lines:?}"
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
            runs: vec![Run {
                text: "Hi".to_owned(),
                bold: true,
                wide: true,
                tall: true,
                accent: true,
            }],
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
        assert!(bytes.windows(2).any(|w| w == *b"Hi"));
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
        second.runs[0].bold = true;
        let bytes = second.document(starprint::starline(), Paper::Mm80, true);
        let bytes = bytes.as_bytes();
        assert!(bytes.windows(5).any(|w| w == b"world"));
        assert!(bytes.ends_with(&[b'\n', 0x1b, b'd', 3]));
    }

    #[test]
    fn typed_line_breaks_are_kept() {
        assert_eq!(wrap_by_words("one\ntwo", 40), ["one", "two"]);
        assert_eq!(wrap_by_words("", 40), [""]);
        assert_eq!(wrap_by_words("one\r\n\ntwo", 40), ["one", "", "two"]);
        assert_eq!(
            plain("\none\n\n").layout::<StarLine>(Paper::Mm80).lines,
            ["", "one", "", ""],
            "blank lines at either end feed paper"
        );
        assert_eq!(wrap_by_words("longword x", 3), ["longword", "x"]);
    }

    #[test]
    fn typed_spaces_are_kept_until_a_line_wraps_at_them() {
        assert_eq!(wrap_by_words("a:  b\n    c ", 40), ["a:  b", "    c"]);
        assert_eq!(
            wrap_by_words("\ta\t b", 40),
            [" a  b"],
            "tabs print as spaces"
        );
        assert_eq!(wrap_by_words(" café  déjà vu ", 9), [" café", "déjà vu"]);
        assert_eq!(
            wrap_by_words("    ab", 5),
            ["ab"],
            "no wider than the paper"
        );
        assert_eq!(
            plain("  indented").layout::<StarLine>(Paper::Mm80).lines,
            ["  indented"]
        );
    }

    #[test]
    fn a_wide_word_takes_two_columns_a_character() {
        let text = Text {
            runs: vec![Run::plain("abcd "), styled("efg", WIDE), Run::plain(" h")],
        };
        let lines = wrap(&text.runs, 10);
        assert_eq!(
            lines,
            [
                vec![Run::plain("abcd")],
                vec![styled("efg", WIDE), Run::plain(" h")],
            ],
            "4 + 1 + 6 is past 10, and 6 + 1 + 1 fits"
        );
    }

    #[test]
    fn a_word_keeps_the_styles_of_its_characters() {
        let bold = Style {
            bold: true,
            ..Style::default()
        };
        let text = Text {
            runs: vec![
                Run::plain("un"),
                styled("bold", bold),
                Run::plain("ed\nnext"),
            ],
        };
        assert_eq!(
            text.layout::<StarLine>(Paper::Mm80).runs,
            [
                vec![Run::plain("un"), styled("bold", bold), Run::plain("ed")],
                vec![Run::plain("next")],
            ]
        );
    }

    #[test]
    fn only_the_styles_that_change_are_sent() {
        let red = Style {
            accent: true,
            ..Style::default()
        };
        let text = Text {
            runs: vec![Run::plain("a"), styled("b", red), Run::plain("c")],
        };
        let doc = text.document(starprint::impact(), Paper::Mm80, false);
        // "a", ESC 4 red, "b", ESC 5 black, "c", the feed.
        assert!(
            doc.as_bytes()
                .ends_with(&[b'a', 0x1b, b'4', b'b', 0x1b, b'5', b'c', b'\n']),
            "{:02x?}",
            doc.as_bytes()
        );
    }

    #[test]
    fn styles_left_on_at_a_line_break_are_put_back_after_the_text() {
        let text = Text {
            runs: vec![Run::plain("a "), styled("b\nc", WIDE)],
        };
        let doc = text.document(starprint::starline(), Paper::Mm80, false);
        assert!(
            doc.as_bytes().ends_with(b"a \x1bW\x01b\nc\x1bW\x00\n"),
            "{:02x?}",
            doc.as_bytes()
        );
    }

    /// Schedules, faxes and API clients from before runs send this shape.
    #[test]
    fn the_plain_shape_is_one_run_both_ways() {
        let text: Text = serde_json::from_str(r#"{"text":"Hi","bold":true}"#).unwrap();
        assert_eq!(
            text.runs,
            [Run {
                bold: true,
                ..Run::plain("Hi")
            }]
        );
        assert_eq!(
            serde_json::to_value(&text).unwrap(),
            serde_json::json!({"text":"Hi","bold":true}),
            "styles that are off are left out"
        );

        let runs = serde_json::json!({"runs":[{"text":"a"},{"text":"b","wide":true}]});
        let text: Text = serde_json::from_value(runs).unwrap();
        assert_eq!(text.runs, [Run::plain("a"), styled("b", WIDE)]);
        assert_eq!(
            serde_json::to_value(&text).unwrap()["runs"][1]["wide"],
            true
        );
    }

    /// A fax's recipient may run a server from before runs, which reads
    /// only `text` and the styles beside it.
    #[test]
    fn runs_carry_their_characters_for_older_readers() {
        #[derive(Deserialize)]
        struct Older {
            text: String,
            #[serde(default)]
            wide: bool,
        }
        let text = Text {
            runs: vec![Run::plain("a\n"), styled("b", WIDE)],
        };
        let json = serde_json::to_value(&text).unwrap();
        let older: Older = serde_json::from_value(json.clone()).unwrap();
        assert_eq!((older.text.as_str(), older.wide), ("a\nb", false));

        let mut edited = json;
        edited["text"] = "ignored".into();
        let again: Text = serde_json::from_value(edited).unwrap();
        assert_eq!(again, text, "the runs are what a new server reads");
    }

    #[test]
    fn a_blank_line_feeds_in_the_style_of_its_break() {
        let tall = Style {
            tall: true,
            ..Style::default()
        };
        let text = Text {
            runs: vec![
                styled("\n", tall),
                Run::plain("A\n\n"),
                styled("B\n\n", tall),
            ],
        };
        assert_eq!(
            text.layout::<StarLine>(Paper::Mm80).runs,
            [
                vec![styled("", tall)],
                vec![Run::plain("A")],
                vec![Run::plain("")],
                vec![styled("B", tall)],
                vec![styled("", tall)],
                vec![styled("", tall)],
            ],
            "and the last, which no break ends, takes the line before's"
        );
    }

    /// Schedules and API clients from before runs send text in one
    /// style, which must print as it always has: every style on, the
    /// lines joined by line feeds, every style off and a feed.
    #[test]
    fn text_in_one_style_prints_the_bytes_it_did_before_runs() {
        let all = Style {
            bold: true,
            wide: true,
            tall: true,
            accent: true,
        };
        for raw in ["Hi", "\nA", "A\n", "A\n\nB", "\n", " a  b \n\tc"] {
            let text = Text {
                runs: vec![styled(raw, all)],
            };
            let lines = wrap_by_words(raw, Builder::<StarLine>::columns(Paper::Mm80) / 2);
            let before = Style::default()
                .switch(all, starprint::starline())
                .text(&lines.join("\n"));
            let before = all.switch(Style::default(), before).raw(b"\n").build();
            assert_eq!(
                text.document(starprint::starline(), Paper::Mm80, false)
                    .as_bytes(),
                before.as_bytes(),
                "{raw:?}"
            );
        }
    }
}
