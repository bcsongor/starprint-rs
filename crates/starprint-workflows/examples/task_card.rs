//! Prints the desktop app's task card through the shared workflow.
//!
//! Due dates accept `now`, `t`/`t0` (today), `t1` (tomorrow), `tn`, `w1`
//! (next Saturday), `w2` (next Sunday), an ISO date, or literal text.
//!
//! Usage: cargo run -p starprint-workflows --example task_card --
//!        <printer-host> thermal|impact "<task>" [priority] [due=DATE]
//!        [density=N]

use chrono::{Datelike, Duration, Local, NaiveDate};
use starprint::transport::TcpTransport;
use starprint_workflows::{Job, Paper, Printer, Speed, TaskCard, check_density};

fn next_weekday(today: NaiveDate, target_from_monday: u32) -> NaiveDate {
    let current = today.weekday().num_days_from_monday();
    let mut days_ahead = (target_from_monday + 7 - current) % 7;
    if days_ahead == 0 {
        days_ahead = 7;
    }
    today + Duration::days(i64::from(days_ahead))
}

fn due(due_by: Option<&str>, today: NaiveDate) -> Option<String> {
    let raw = due_by.map(str::trim).filter(|value| !value.is_empty())?;
    let lower = raw.to_ascii_lowercase();

    let parsed = if matches!(lower.as_str(), "now" | "t" | "t0") {
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

    Some(parsed.map_or_else(|| raw.to_owned(), |date| date.to_string()))
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
        flag.as_str() != "priority"
            && !["due=", "density="]
                .iter()
                .any(|prefix| flag.starts_with(prefix))
    }) {
        return Err(format!("unknown option {bad:?}; {USAGE}").into());
    }

    let value = |key: &str| flags.iter().find_map(|flag| flag.strip_prefix(key));
    let density: Option<i8> = value("density=").map(str::parse).transpose()?;
    if let Some(density) = density {
        check_density(density)?;
    }
    let printer = match kind.as_str() {
        "thermal" => Printer::thermal(
            host.clone(),
            9100,
            Paper::Mm80,
            density.unwrap_or(3),
            Speed::Slow,
        ),
        "impact" if density.is_none() => Printer::impact(host.clone(), 9100),
        "impact" => return Err("density is only available for thermal printers".into()),
        _ => return Err(USAGE.into()),
    };
    let job = Job::TaskCard(TaskCard {
        text,
        priority: flags.iter().any(|flag| flag == "priority"),
        due: due(value("due="), Local::now().date_naive()),
    });
    let document = printer.document(&job, None)?;

    let mut transport = TcpTransport::connect(&host)?;
    transport.print(&document)?;
    println!("sent {} bytes to {host}", document.as_bytes().len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()
    }

    #[test]
    fn shortcuts_become_dates_the_workflow_understands() {
        let today = fixed_date();
        assert_eq!(due(Some("t0"), today).as_deref(), Some("2025-01-15"));
        assert_eq!(due(Some("t1"), today).as_deref(), Some("2025-01-16"));
        assert_eq!(due(Some("t10"), today).as_deref(), Some("2025-01-25"));
        assert_eq!(due(Some("w1"), today).as_deref(), Some("2025-01-18"));
        assert_eq!(due(Some("w2"), today).as_deref(), Some("2025-01-19"));
        assert_eq!(due(Some("someday"), today).as_deref(), Some("someday"));
        assert_eq!(due(Some("  "), today), None);
    }
}
