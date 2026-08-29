//! A slip to write on by hand. The Python GUI's `note_slip` printed the
//! date and fed 24 blank lines; this one keeps the date and rules the
//! paper at a pitch in millimetres.
//!
//! The ruling is drawn as dots rather than underlined spaces, because a
//! pitch in millimetres and the vertical lines of squared paper are not
//! something the character grid can express.

use chrono::Local;
use serde::{Deserialize, Serialize};
use starprint::graphics::{BitImage, Bitmap, Density, DeviceProfile};
use starprint::{Builder, Cut, Document, Impact, LineSpacing, Protocol, RasterQuality, StarLine};

use crate::Paper;
use crate::task_card::{align_right, format_date};
use crate::text::TextStyle;

/// What is drawn for the hand to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rule {
    /// Bare paper, as the Python GUI printed.
    Blank,
    /// A dot at each corner of the grid.
    Dots,
    /// A rule under each row.
    Lines,
    /// Squared paper: rules both ways.
    Squares,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub rule: Rule,
    /// Rows to write in.
    pub rows: u8,
    /// Millimetres between the rules, across and down.
    pub pitch: u8,
}

/// The slip as it will print, for the preview. The ruling is geometry
/// the preview draws for itself, in the millimetres it was asked for.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub columns: usize,
    /// Right-aligned to fill the header line, as the Python GUI had it.
    pub date: String,
}

/// A slip longer than this is a roll, not a note.
const MAX_ROWS: u8 = 20;
/// Tight enough for squared paper, wide enough for a large hand.
const MIN_PITCH_MM: u8 = 4;
const MAX_PITCH_MM: u8 = 12;

/// A grid dot, in millimetres. A rule is a single dot thick, which the
/// thermal head blooms to about 0.19 mm.
const DOT_MM: f32 = 0.3;

const MM_PER_INCH: f32 = 25.4;

/// What differs between the heads when ruling paper.
pub trait NoteStyle: TextStyle + Sized {
    /// The head and roll as geometry.
    fn profile(paper: Paper) -> &'static DeviceProfile;

    /// Feeds blank paper, near enough for a slip nobody measures.
    fn feed_mm(self, mm: f32) -> Self;

    /// Draws the ruling, which is a raster on one head and a bit image
    /// on the other.
    fn rule(self, grid: Bitmap) -> Self;
}

impl NoteStyle for Builder<StarLine> {
    fn profile(paper: Paper) -> &'static DeviceProfile {
        match paper {
            Paper::Mm80 => &DeviceProfile::THERMAL_80MM,
            Paper::Mm112 => &DeviceProfile::THERMAL_112MM,
        }
    }

    fn feed_mm(self, mm: f32) -> Self {
        // The pitch is selected rather than assumed, so the feed is the
        // length asked for. `ESC @` puts it back for the next job.
        self.line_spacing(LineSpacing::FourMm).feed(lines(mm, 4.0))
    }

    fn rule(self, grid: Bitmap) -> Self {
        self.raster(grid, RasterQuality::High)
    }
}

impl NoteStyle for Builder<Impact> {
    fn profile(_paper: Paper) -> &'static DeviceProfile {
        &DeviceProfile::SP700
    }

    fn feed_mm(self, mm: f32) -> Self {
        // The SP700 has no line-pitch command here; `ESC @` leaves 1/6".
        self.feed(lines(mm, MM_PER_INCH / 6.0))
    }

    fn rule(self, grid: Bitmap) -> Self {
        let image = BitImage::new(grid, Density::Single).expect("sized for the SP700 head");
        self.bit_image(&image)
    }
}

/// Line feeds covering `mm` at `pitch`, clamped to what `ESC a` takes.
fn lines(mm: f32, pitch: f32) -> u8 {
    (mm / pitch).round().clamp(1.0, 127.0) as u8
}

impl Note {
    /// Clamped, so a dragged slider cannot ask for a roll of paper.
    fn rows(&self) -> u32 {
        u32::from(self.rows.clamp(1, MAX_ROWS))
    }

    fn pitch(&self) -> f32 {
        f32::from(self.pitch.clamp(MIN_PITCH_MM, MAX_PITCH_MM))
    }

    fn grid(&self, profile: &DeviceProfile) -> Bitmap {
        ruling(self.rule, self.rows(), self.pitch(), profile)
    }

    pub fn layout<P: Protocol>(&self, paper: Paper) -> Layout
    where
        Builder<P>: TextStyle,
    {
        let columns = <Builder<P> as TextStyle>::columns(paper);
        Layout {
            columns,
            date: align_right(&format_date(Local::now().date_naive()), columns),
        }
    }

    /// Without `cut` the slip only feeds clear of the head.
    pub fn document<P: Protocol>(&self, builder: Builder<P>, paper: Paper, cut: bool) -> Document
    where
        Builder<P>: NoteStyle,
    {
        let layout = self.layout::<P>(paper);
        let doc = builder.line(&layout.date).feed(1);

        let doc = match self.rule {
            Rule::Blank => doc.feed_mm(self.rows() as f32 * self.pitch()),
            _ => doc.rule(self.grid(<Builder<P> as NoteStyle>::profile(paper))),
        };

        if cut {
            doc.cut(Cut::FeedThenPartial).build()
        } else {
            doc.feed(3).build()
        }
    }
}

/// The ruling as dots. The two heads have different dot pitches, and the
/// impact head's is not even square, so everything is placed from
/// millimetres.
fn ruling(rule: Rule, rows: u32, pitch_mm: f32, profile: &DeviceProfile) -> Bitmap {
    let width = profile.width_dots_single;
    let across = profile.horizontal_dpi as f32 / MM_PER_INCH;
    let down = profile.vertical_dpi as f32 / MM_PER_INCH;

    let pitch_x = pitch_mm * across;
    let pitch_y = pitch_mm * down;
    let height = ((rows as f32 * pitch_y).round() as u32).max(1);
    // Whole squares, centred, so the ruling is not lopsided.
    let squares = (width as f32 / pitch_x) as u32;
    let margin = (width as f32 - squares as f32 * pitch_x) / 2.0;

    let (thick_x, thick_y) = match rule {
        Rule::Dots => (thickness(across), thickness(down)),
        _ => (1, 1),
    };

    // Ruled paper is written on top of the rule, so it has none at the
    // top; a grid is closed on all sides.
    let mut ruled = vec![false; height as usize];
    let first = u32::from(rule == Rule::Lines);
    for row in first..=rows {
        mark(&mut ruled, (row as f32 * pitch_y).round() as usize, thick_y);
    }

    let mut vertical = vec![false; width as usize];
    if rule != Rule::Lines {
        for square in 0..=squares {
            mark(
                &mut vertical,
                (margin + square as f32 * pitch_x).round() as usize,
                thick_x,
            );
        }
    }

    // Squared paper is a closed block, so its rules stop at the
    // outermost sides; running on to the edge of the paper would leave
    // them sticking out past the grid.
    let sides = match (
        vertical.iter().position(|&on| on),
        vertical.iter().rposition(|&on| on),
    ) {
        (Some(first), Some(last)) => first..=last,
        // Only when nothing is drawn, which is every rule but these two.
        _ => 1..=0,
    };

    Bitmap::from_fn(width, height, |x, y| {
        let (x, y) = (x as usize, y as usize);
        match rule {
            Rule::Blank => false,
            Rule::Dots => ruled[y] && vertical[x],
            Rule::Lines => ruled[y],
            Rule::Squares => (ruled[y] && sides.contains(&x)) || vertical[x],
        }
    })
}

fn thickness(dots_per_mm: f32) -> usize {
    ((DOT_MM * dots_per_mm).round() as usize).max(1)
}

/// Inks `thickness` cells from `at`, pulled back to fit rather than
/// clipped, so the last rule of a slip is whole.
fn mark(mask: &mut [bool], at: usize, thickness: usize) {
    let start = at.min(mask.len().saturating_sub(thickness));
    let end = (start + thickness).min(mask.len());
    for cell in &mut mask[start..end] {
        *cell = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(rule: Rule, rows: u8, pitch: u8) -> Note {
        Note { rule, rows, pitch }
    }

    fn thermal(note: &Note) -> Bitmap {
        note.grid(&DeviceProfile::THERMAL_80MM)
    }

    fn ink(grid: &Bitmap, y: u32) -> Vec<u32> {
        (0..grid.width()).filter(|&x| grid.get(x, y)).collect()
    }

    #[test]
    fn a_ruling_is_as_deep_as_the_rows_it_holds() {
        // 8 mm at 8 dots/mm.
        let grid = thermal(&note(Rule::Lines, 6, 8));
        assert_eq!(grid.width(), 576, "the whole print region");
        assert_eq!(grid.height(), 384, "6 rows of 64 dots");
    }

    #[test]
    fn lines_rule_under_each_row_and_not_above_the_first() {
        let grid = thermal(&note(Rule::Lines, 3, 8));
        let ruled: Vec<u32> = (0..grid.height()).filter(|&y| grid.get(0, y)).collect();
        assert_eq!(ruled, [64, 128, 191], "the last is pulled inside");
        for y in ruled {
            assert_eq!(ink(&grid, y).len(), 576, "a rule runs edge to edge");
        }
    }

    #[test]
    fn squares_are_closed_on_all_sides_and_the_same_pitch_both_ways() {
        let grid = thermal(&note(Rule::Squares, 2, 5));
        // 5 mm at 8 dots/mm: rules at 0, 40 and 79.
        assert!(grid.get(8, 0) && grid.get(8, 40) && grid.get(8, 79));
        // 576 / 40 = 14 squares with 16 dots left over, so 8 each side.
        let top = ink(&grid, 0);
        assert_eq!(
            (top.first(), top.last(), top.len()),
            (Some(&8), Some(&568), 561),
            "the rule is solid between the outermost sides, and stops there"
        );
        let between = ink(&grid, 20);
        assert_eq!(between.first(), Some(&8), "centred, not flush left");
        assert_eq!(between.len(), 15, "14 squares have 15 sides");
        assert_eq!(
            between.windows(2).map(|w| w[1] - w[0]).max(),
            Some(40),
            "one pitch apart"
        );
    }

    #[test]
    fn dots_mark_the_corners_only() {
        let grid = thermal(&note(Rule::Dots, 2, 5));
        // 0.3 mm at 8 dots/mm is a dot of 2.
        assert_eq!(ink(&grid, 0), ink(&grid, 1), "two rows deep");
        assert_eq!(ink(&grid, 0).len(), 30, "15 corners, two dots wide");
        assert!(ink(&grid, 20).is_empty(), "nothing between the rows");
    }

    #[test]
    fn the_impact_head_rules_the_same_millimetres_on_its_own_grid() {
        let grid = note(Rule::Squares, 4, 5).grid(&DeviceProfile::SP700);
        assert_eq!(grid.width(), 210, "63 mm at the head's pitch");
        // 72 DPI down: 5 mm is 14.2 rows.
        assert_eq!(grid.height(), 57, "4 rows of 5 mm");
        // 210 dots over 63 mm: 5 mm is 16.7 dots.
        let between = ink(&grid, 7);
        assert_eq!(between.len(), 13, "12 squares across the 63 mm");
    }

    #[test]
    fn a_blank_slip_feeds_the_paper_it_would_have_ruled() {
        let bytes = note(Rule::Blank, 6, 8)
            .document(starprint::impact(), Paper::Mm80, true)
            .as_bytes()
            .to_vec();
        assert!(
            !bytes.windows(2).any(|w| w == [0x1b, b'^']),
            "no bit image: {bytes:02x?}"
        );
        // 48 mm at 1/6" a line.
        assert!(bytes.windows(3).any(|w| w == [0x1b, b'a', 11]));

        let bytes = note(Rule::Blank, 6, 8)
            .document(starprint::starline(), Paper::Mm80, true)
            .as_bytes()
            .to_vec();
        // 48 mm at the 4 mm pitch it selects.
        assert!(bytes.windows(3).any(|w| w == [0x1b, b'z', 1]));
        assert!(bytes.windows(3).any(|w| w == [0x1b, b'a', 12]));
    }

    #[test]
    fn the_date_always_fills_the_header_line() {
        let layout = note(Rule::Lines, 1, 8).layout::<StarLine>(Paper::Mm112);
        assert_eq!(layout.date.chars().count(), 69, "across the roll");
        assert_eq!(
            layout.date.trim(),
            format_date(Local::now().date_naive()),
            "the task card's date, so a stack of paper reads the same"
        );
    }

    #[test]
    fn the_header_is_the_date_and_one_blank_line() {
        let note = note(Rule::Lines, 1, 7);
        let layout = note.layout::<Impact>(Paper::Mm80);
        let bytes = note
            .document(starprint::impact(), Paper::Mm80, true)
            .as_bytes()
            .to_vec();

        let mut expected = layout.date.into_bytes();
        // The line feed, then a blank line, then straight into the ruling.
        expected.extend([b'\n', 0x1b, b'a', 1, 0x1b, b'3', 27]);
        assert!(
            bytes.windows(expected.len()).any(|w| w == expected),
            "{bytes:02x?}"
        );
        assert!(bytes.ends_with(&[0x1b, b'd', 3]), "ends with the cut");
    }

    #[test]
    fn the_slip_is_clamped_to_a_slip() {
        let tall = thermal(&note(Rule::Lines, 250, 250));
        let most = thermal(&note(Rule::Lines, MAX_ROWS, MAX_PITCH_MM));
        assert_eq!(tall.height(), most.height());
        let short = thermal(&note(Rule::Lines, 0, 0));
        assert_eq!(short.height(), thermal(&note(Rule::Lines, 1, 4)).height());
    }

    #[test]
    fn without_a_cut_the_slip_feeds_clear_of_the_head() {
        let bytes = note(Rule::Lines, 1, 8)
            .document(starprint::impact(), Paper::Mm80, false)
            .as_bytes()
            .to_vec();
        assert!(bytes.ends_with(&[0x1b, b'a', 3]), "{bytes:02x?}");
    }
}
