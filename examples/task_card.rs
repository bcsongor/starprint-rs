//! Prints a task card equivalent to the sibling `starprint` desktop GUI.
//!
//! The task is bold and quad-size; high priority is red on an SP700 and
//! inverse on a thermal printer.
//! Due dates accept `now`, `t`/`t0` (today), `t1` (tomorrow), `tn`, `w1`
//! (next Saturday), `w2` (next Sunday), an ISO date, or literal text.
//!
//! Usage: cargo run --example task_card -- <printer-host> thermal|impact
//!        "<task>" [priority] [due=DATE] [density=N]

use chrono::{Datelike, Duration, Local, NaiveDate};
use starprint::transport::TcpTransport;
use starprint::{Builder, Color, Cut, Document, Impact, PrintSpeed, Protocol, StarLine};

const PRIORITY_TEXT: &str = " HIGH PRIORITY ";

trait TaskPrinter: Sized {
    const COLUMNS: usize;

    fn set_wide(self, on: bool) -> Self;
    fn set_tall(self, on: bool) -> Self;
    fn set_accent(self, on: bool) -> Self;
}

impl TaskPrinter for Builder<StarLine> {
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

impl TaskPrinter for Builder<Impact> {
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
        lines.push(if current.is_empty() && words.is_empty() {
            String::new()
        } else {
            current
        });
    }
    lines
}

fn next_weekday(today: NaiveDate, target_from_monday: u32) -> NaiveDate {
    let current = today.weekday().num_days_from_monday();
    let mut days_ahead = (target_from_monday + 7 - current) % 7;
    if days_ahead == 0 {
        days_ahead = 7;
    }
    today + Duration::days(i64::from(days_ahead))
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

fn parse_due_by(due_by: Option<&str>, today: NaiveDate) -> String {
    let Some(raw) = due_by.map(str::trim).filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let lower = raw.to_ascii_lowercase();

    let parsed = if matches!(lower.as_str(), "now" | "t") {
        Some(today)
    } else if let Some(offset) = lower.strip_prefix('t') {
        (!offset.is_empty() && offset.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| offset.parse::<i64>().ok())
            .flatten()
            .and_then(|days| today.checked_add_signed(Duration::days(days)))
    } else if lower == "w1" {
        Some(next_weekday(today, 5))
    } else if lower == "w2" {
        Some(next_weekday(today, 6))
    } else {
        NaiveDate::parse_from_str(&lower, "%Y-%m-%d").ok()
    };

    parsed.map_or_else(|| raw.to_owned(), format_date)
}

fn align_right(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(text.chars().count());
    format!("{}{text}", " ".repeat(padding))
}

fn task_card<P: Protocol>(
    builder: Builder<P>,
    text: &str,
    priority: bool,
    due_by: Option<&str>,
    today: NaiveDate,
) -> Document
where
    Builder<P>: TaskPrinter,
{
    let columns = <Builder<P> as TaskPrinter>::COLUMNS;
    let due = parse_due_by(due_by, today);
    let mut card = builder.bold(true);
    if priority || !due.is_empty() {
        if priority {
            card = card.set_accent(true).text(PRIORITY_TEXT).set_accent(false);
        }
        if !due.is_empty() {
            let width = columns.saturating_sub(if priority {
                PRIORITY_TEXT.chars().count()
            } else {
                0
            });
            card = card.bold(false).text(&align_right(&due, width)).bold(true);
        }
        card = card.raw([b'\n']).feed(1);
    }

    card.set_wide(true)
        .set_tall(true)
        .text(&wrap_by_words(text, columns / 2).join("\n"))
        .feed(2)
        .cut(Cut::FeedThenPartial)
        .build()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const USAGE: &str = "usage: task_card <printer-host> thermal|impact \"<task>\" [priority] [due=DATE] [density=N]";
    let mut args = std::env::args().skip(1);
    let host = args.next().ok_or(USAGE)?;
    let kind = args.next().ok_or(USAGE)?;
    let text = args.next().ok_or(USAGE)?;
    if text.trim().is_empty() {
        return Err("task text cannot be empty".into());
    }
    let flags: Vec<String> = args.collect();
    if let Some(bad) = flags.iter().find(|flag| {
        !matches!(flag.as_str(), "priority" | "slow")
            && !["due=", "density="]
                .iter()
                .any(|prefix| flag.starts_with(prefix))
    }) {
        return Err(format!("unknown option {bad:?}; {USAGE}").into());
    }

    let value = |key: &str| flags.iter().find_map(|flag| flag.strip_prefix(key));
    let priority = flags.iter().any(|flag| flag == "priority");
    let due_by = value("due=");
    let density: Option<i8> = value("density=").map(str::parse).transpose()?;
    let today = Local::now().date_naive();

    let doc = match kind.as_str() {
        "thermal" => {
            let builder = starprint::starline()
                .print_speed(PrintSpeed::Slow)
                .print_density(density.unwrap_or(3));
            task_card(builder, &text, priority, due_by, today)
        }
        "impact" => task_card(starprint::impact(), &text, priority, due_by, today),
        _ => return Err(USAGE.into()),
    };

    let mut printer = TcpTransport::connect(&host)?;
    printer.print(&doc)?;
    println!("sent {} bytes to {host}", doc.as_bytes().len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(hex: &str) -> Vec<u8> {
        hex.split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }

    fn fixed_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()
    }

    #[test]
    fn basic_impact_card_matches_gui_fixture() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 57 01 1b 68 01 54 65 73 74 20 74 61 73 6b 1b 61 02 1b 64 03",
        );
        let actual = task_card(starprint::impact(), "Test task", false, None, fixed_date());
        assert_eq!(actual.as_bytes(), expected);
    }

    #[test]
    fn priority_card_emphasises_header_and_separates_task() {
        let expected = bytes(
            "1b 40 1b 1d 74 01 1b 45 1b 34 20 48 49 47 48 20 50 52 49 4f 52 49 54 59 20 1b 35 1b 46 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 31 35 20 4a 41 4e 20 32 30 32 35 1b 45 0a 1b 61 01 1b 57 01 1b 68 01 55 72 67 65 6e 74 20 74 61 73 6b 1b 61 02 1b 64 03",
        );
        let actual = task_card(
            starprint::impact(),
            "Urgent task",
            true,
            Some("now"),
            fixed_date(),
        );
        assert_eq!(actual.as_bytes(), expected);
    }

    #[test]
    fn parses_all_gui_due_date_shortcuts() {
        let today = fixed_date();
        assert_eq!(parse_due_by(Some("t0"), today), "15 JAN 2025");
        assert_eq!(parse_due_by(Some("t1"), today), "16 JAN 2025");
        assert_eq!(parse_due_by(Some("t10"), today), "25 JAN 2025");
        assert_eq!(parse_due_by(Some("w1"), today), "18 JAN 2025");
        assert_eq!(parse_due_by(Some("w2"), today), "19 JAN 2025");
        assert_eq!(parse_due_by(Some("2025-02-03"), today), "03 FEB 2025");
        assert_eq!(parse_due_by(Some("someday"), today), "someday");
        assert_eq!(parse_due_by(Some("t-1"), today), "t-1");
    }

    #[test]
    fn wraps_words_at_quad_size_width() {
        assert_eq!(
            wrap_by_words("one two three four", 10),
            ["one two", "three four"]
        );
    }
}
