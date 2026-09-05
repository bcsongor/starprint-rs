//! Ready-made jobs on top of the `starprint` crate: a printer profile, a
//! tagged [`Job`] and the bytes each one prints.
//!
//! The desktop app and the HTTP API are both thin adapters over this
//! crate, so a task card printed from either is byte for byte the same.
//! Nothing here reads a file or opens a socket. A picture job is given
//! its image, and the caller sends the finished [`Document`].
//!
//! [`Document`]: starprint::Document

mod printer;

pub mod note;
pub mod picture;
pub mod qr;
pub mod task_card;
pub mod test_page;
pub mod text;

use serde::Deserialize;
use starprint::{Builder, Cut, Document, Protocol};

pub use note::Note;
pub use picture::Picture;
pub use printer::{Head, Paper, Printer, PrinterKind, Speed, TWO_COLOR_DENSITY, check_density};
pub use qr::Qr;
pub use task_card::TaskCard;
pub use test_page::TestPage;
pub use text::Text;

/// Both heads place dots on a pitch of their own, so anything with a
/// size in millimetres converts through this rather than a dot count.
pub(crate) const MM_PER_INCH: f32 = 25.4;

/// How a job ends: a cut, or three blank lines for the next job to start
/// under when the profile has cutting off.
pub(crate) fn finish<P: Protocol>(doc: Builder<P>, cut: bool) -> Document {
    if cut {
        doc.cut(Cut::FeedThenPartial).build()
    } else {
        doc.feed(3).build()
    }
}

/// Pictures and QR codes need two blank lines before a cut. Without a
/// cut, the feed in `finish` supplies the margin.
pub(crate) fn finish_graphic<P: Protocol>(doc: Builder<P>, cut: bool) -> Document {
    finish(if cut { doc.feed(2) } else { doc }, cut)
}

/// The six jobs, as one tagged enum. `kind` picks the variant and the
/// rest of the object is that job's own settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Job {
    TaskCard(TaskCard),
    Text(Text),
    Note(Note),
    Qr(Qr),
    TestPage(TestPage),
    Picture(Picture),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::Rule;
    use crate::picture::Dither;
    use crate::qr::Ecc;

    /// A caller leaves out whatever it does not care about.
    #[test]
    fn jobs_deserialise_from_their_content_alone() {
        let job: Job = serde_json::from_str(r#"{"kind":"task-card","text":"Buy milk"}"#).unwrap();
        let Job::TaskCard(card) = job else {
            panic!("task card")
        };
        assert_eq!(card.text, "Buy milk");
        assert!(!card.priority);
        assert_eq!(card.reference, None);
        assert_eq!(card.due, None);

        let job: Job = serde_json::from_str(r#"{"kind":"test-page"}"#).unwrap();
        assert!(matches!(
            job,
            Job::TestPage(TestPage {
                double_resolution: false
            })
        ));

        let job: Job = serde_json::from_str(r#"{"kind":"note"}"#).unwrap();
        let Job::Note(note) = job else { panic!("note") };
        assert_eq!((note.rule, note.rows, note.pitch), (Rule::Lines, 10, 7));

        let job: Job = serde_json::from_str(r#"{"kind":"picture"}"#).unwrap();
        let Job::Picture(picture) = job else {
            panic!("picture")
        };
        assert_eq!(picture, Picture::default());

        let job: Job = serde_json::from_str(r#"{"kind":"qr","data":"https://x.test"}"#).unwrap();
        let Job::Qr(code) = job else { panic!("qr") };
        assert_eq!(code.data, "https://x.test");
        assert_eq!(
            (code.caption, code.error_correction, code.size, code.radius),
            (None, Ecc::M, 30, 0)
        );
    }

    #[test]
    fn a_setting_that_is_given_wins_over_the_default() {
        let job: Job =
            serde_json::from_str(r#"{"kind":"picture","dither":"bayer","brightness":1.4}"#)
                .unwrap();
        let Job::Picture(picture) = job else {
            panic!("picture")
        };
        assert_eq!(picture.dither, Dither::Bayer);
        assert_eq!(picture.brightness, 1.4);
        assert_eq!(picture.threshold, 128, "and the rest still default");
    }

    #[test]
    fn an_unknown_kind_is_refused() {
        assert!(serde_json::from_str::<Job>(r#"{"kind":"receipt"}"#).is_err());
        assert!(serde_json::from_str::<Job>(r#"{"text":"no kind"}"#).is_err());
    }
}
