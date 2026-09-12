//! Schedules: a job request, a printer and a cron expression, and the
//! loop that prints each one on the server's clock.
//!
//! Scope is deliberately small: no history, no catching up on a missed
//! minute, no retries.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Days, Local, TimeDelta, Timelike};
use croner::Cron;
use croner::errors::CronError;
use croner::parser::{CronParser, Seconds};
use serde::{Deserialize, Serialize};
use starprint_workflows::Job;

use crate::data::Data;
use crate::job::JobRequest;
use crate::printers::{self, Printers};
use crate::problem::Problem;

/// The due date a scheduled task card gets when it prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Due {
    RunDay,
    NextDay,
}

impl Due {
    pub fn date(self, at: DateTime<Local>) -> String {
        let days = match self {
            Self::RunDay => 0,
            Self::NextDay => 1,
        };
        (at.date_naive() + Days::new(days))
            .format("%Y-%m-%d")
            .to_string()
    }
}

/// A schedule as a client sends one: a job request as `/jobs` takes
/// it, with the printer it goes to and when.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleSpec {
    pub printer: String,
    /// Five fields, in the server's local time.
    pub cron: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// Task cards only: the form's date does not carry over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<Due>,
    /// Prints with the profile's settings; a schedule carries no
    /// overrides, so a profile change cannot leave it unable to print.
    pub job: Job,
}

fn enabled() -> bool {
    true
}

/// Five fields; a seconds field is refused.
pub fn parse_cron(expression: &str) -> Result<Cron, CronError> {
    CronParser::builder()
        .seconds(Seconds::Disallowed)
        .build()
        .parse(expression)
}

impl ScheduleSpec {
    /// Checks the stored definition without requiring its printer to
    /// exist. Deleting a profile leaves its schedules in place.
    pub fn check(&self) -> Result<(), Problem> {
        parse_cron(&self.cron)
            .map_err(|e| Problem::bad_request(format!("`cron` is not a schedule: {e}.")))?;
        if self.job.needs_image() {
            return Err(Problem::bad_request(
                "A picture cannot be scheduled: the server holds no image to print it from.",
            ));
        }
        if self.due.is_some() && !matches!(self.job, Job::TaskCard(_)) {
            return Err(Problem::bad_request(
                "`due` dates a task card; this job has no date to set.",
            ));
        }
        Ok(())
    }

    /// Whether this fires in the minute `now` falls in.
    fn is_due(&self, now: DateTime<Local>) -> bool {
        // Subtracted rather than set with `with_second`, which resolves
        // the result as a local time and has no answer during the hour
        // the clocks go back.
        let minute = now
            - TimeDelta::seconds(now.second().into())
            - TimeDelta::nanoseconds(now.nanosecond().into());
        self.enabled
            && parse_cron(&self.cron)
                .and_then(|cron| cron.is_time_matching(&minute))
                .unwrap_or(false)
    }

    /// The request as it prints at `at`: a task card takes its due
    /// date then.
    pub fn request_at(&self, at: DateTime<Local>) -> JobRequest {
        let mut job = self.job.clone();
        if let Job::TaskCard(card) = &mut job {
            card.due = self.due.map(|due| due.date(at));
        }
        JobRequest {
            job,
            cut: None,
            density: None,
            speed: None,
        }
    }
}

/// Builds the job before accepting a schedule, using the same checks
/// as a manual job without opening a printer connection.
pub async fn admit(spec: &ScheduleSpec, data: &Data) -> Result<(), Problem> {
    spec.check()?;
    let profile = data
        .profile(&spec.printer)
        .ok_or_else(|| Problem::bad_request(format!("No printer named `{}`.", spec.printer)))?;
    spec.request_at(Local::now())
        .document(&profile.printer, None)
        .await?;
    Ok(())
}

/// The schedules that fire in the minute `now` falls in, by id.
pub fn due(data: &Data, now: DateTime<Local>) -> Vec<(String, ScheduleSpec)> {
    data.schedules()
        .into_iter()
        .filter(|(_, spec)| spec.is_due(now))
        .collect()
}

/// Prints one schedule as it stands at `now`. A schedule whose printer
/// is gone, or has no host yet, does nothing.
pub async fn run(
    printers: &Printers,
    spec: &ScheduleSpec,
    now: DateTime<Local>,
) -> Result<usize, Problem> {
    let Some(profile) = printers.data.profile(&spec.printer) else {
        return Ok(0);
    };
    let printer = profile.printer;
    if printer.host.trim().is_empty() {
        return Ok(0);
    }
    let payload = spec.request_at(now).document(&printer, None).await?;
    printers::send(&printer, payload, &printers.queue).await
}

/// Runs the schedules that are due, each time the minute turns, for as
/// long as the future is polled. Every job is its own task, so one
/// waiting on a busy printer holds up neither the others nor the loop.
/// Dropping the future cancels pending jobs. Blocking writes that have
/// already started finish under their queue guard.
pub async fn serve(printers: Arc<Printers>) {
    let mut jobs = tokio::task::JoinSet::new();
    loop {
        let wait = 60_000 - Local::now().timestamp_millis().rem_euclid(60_000);
        let sleep = tokio::time::sleep(Duration::from_millis(wait as u64));
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                () = &mut sleep => break,
                Some(_) = jobs.join_next(), if !jobs.is_empty() => {},
            }
        }
        let now = Local::now();
        for (id, spec) in due(&printers.data, now) {
            let printers = Arc::clone(&printers);
            jobs.spawn(async move {
                if let Err(problem) = run(&printers, &spec, now).await {
                    eprintln!(
                        "starprint-api: schedule {id} on {}: {}",
                        spec.printer,
                        problem.detail()
                    );
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;
    use crate::printers::PrintQueue;
    use chrono::TimeZone;
    use serde_json::json;
    use starprint_workflows::Printer;
    use tokio::io::AsyncReadExt as _;
    use tokio::net::TcpListener;

    fn spec(printer: &str, cron: &str, due: Option<Due>) -> ScheduleSpec {
        ScheduleSpec {
            printer: printer.to_owned(),
            cron: cron.to_owned(),
            enabled: true,
            due,
            job: serde_json::from_value(
                json!({ "kind": "task-card", "text": "Standup", "due": "2026-01-01" }),
            )
            .unwrap(),
        }
    }

    fn data(port: u16) -> Data {
        Data::ephemeral(
            vec![Profile {
                name: "sp743".to_owned(),
                printer: Printer::impact("127.0.0.1".to_owned(), port),
                notes: String::new(),
            }],
            "t".to_owned(),
        )
    }

    fn at(day: u32, hour: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 9, day, hour, 0, 0).unwrap()
    }

    #[test]
    fn a_spec_reads_from_a_job_request_with_a_printer_and_a_time() {
        let spec: ScheduleSpec = serde_json::from_value(json!({
            "printer": "sp743",
            "cron": "0 9 * * 1-5",
            "job": { "kind": "text", "text": "Hi" },
        }))
        .unwrap();
        assert!(spec.enabled, "on unless said otherwise");
        assert_eq!(spec.due, None);
        let back = serde_json::to_value(&spec).unwrap();
        assert_eq!(back["job"]["kind"], "text");
        assert!(back.get("due").is_none(), "an unset rule is left out");

        let with_density =
            json!({ "printer": "p", "cron": "* * * * *", "job": { "kind": "note" }, "density": 4 });
        assert!(
            serde_json::from_value::<ScheduleSpec>(with_density).is_err(),
            "no overrides: the profile decides"
        );

        let with_id =
            json!({ "id": "x", "printer": "p", "cron": "* * * * *", "job": { "kind": "note" } });
        assert!(serde_json::from_value::<ScheduleSpec>(with_id).is_err());
    }

    #[test]
    fn a_spec_is_checked_before_it_is_kept() {
        let detail = |spec: ScheduleSpec| {
            serde_json::to_value(spec.check().unwrap_err()).unwrap()["detail"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        assert!(spec("sp743", "0 9 * * *", None).check().is_ok());
        for cron in ["0 9 * *", "*/5 * * * * *", "0 0 9 * * * 2026"] {
            assert!(
                detail(spec("sp743", cron, None)).starts_with("`cron` is not a schedule"),
                "{cron}"
            );
        }
        let mut picture = spec("sp743", "0 9 * * *", None);
        picture.job = serde_json::from_value(json!({ "kind": "picture" })).unwrap();
        assert!(detail(picture).starts_with("A picture cannot be scheduled"));
        let mut dated_text = spec("sp743", "0 9 * * *", Some(Due::RunDay));
        dated_text.job = serde_json::from_value(json!({ "kind": "text", "text": "Hi" })).unwrap();
        assert!(detail(dated_text).starts_with("`due` dates a task card"));
    }

    #[test]
    fn schedules_fire_in_their_minute_when_enabled() {
        let data = data(9100);
        let nine = data.add_schedule(spec("sp743", "0 9 * * *", None)).unwrap();
        let mut off = spec("sp743", "0 9 * * *", None);
        off.enabled = false;
        data.add_schedule(off).unwrap();
        data.add_schedule(spec("sp743", "0 10 * * *", None))
            .unwrap();

        assert!(due(&data, at(8, 8)).is_empty());
        let fired = due(&data, at(8, 9).with_second(17).unwrap());
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].0, nine);
        assert_eq!(due(&data, at(8, 10)).len(), 1);
    }

    #[test]
    fn a_task_card_is_dated_when_it_runs() {
        let due = |rule| {
            let Job::TaskCard(card) = spec("sp743", "0 9 * * *", rule).request_at(at(8, 9)).job
            else {
                panic!("task card")
            };
            card.due
        };
        assert_eq!(due(None), None, "the form's date does not carry over");
        assert_eq!(due(Some(Due::RunDay)).as_deref(), Some("2026-09-08"));
        assert_eq!(due(Some(Due::NextDay)).as_deref(), Some("2026-09-09"));
    }

    #[tokio::test(start_paused = true)]
    async fn stopping_the_scheduler_cancels_pending_jobs() {
        use std::future::Future as _;
        use std::task::{Context, Waker};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let data = Arc::new(data(port));
        data.add_schedule(spec("sp743", "* * * * *", None)).unwrap();
        let queue = Arc::new(PrintQueue::default());
        let turn = queue.lock("127.0.0.1", port).await;
        let printers = Arc::new(Printers::new(data, Arc::clone(&queue)));
        let mut scheduler = Box::pin(serve(printers));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(scheduler.as_mut().poll(&mut cx).is_pending());
        tokio::time::advance(Duration::from_secs(60)).await;
        assert!(scheduler.as_mut().poll(&mut cx).is_pending());

        tokio::time::resume();
        drop(scheduler);
        tokio::task::yield_now().await;
        drop(turn);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_run_prints_through_the_queue_and_skips_a_missing_printer() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let received = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            bytes
        });
        let printers = Printers::new(Arc::new(data(port)), Arc::new(PrintQueue::default()));
        let sent = run(
            &printers,
            &spec("sp743", "* * * * *", Some(Due::RunDay)),
            at(8, 9),
        )
        .await
        .unwrap();
        let bytes = received.await.unwrap();
        assert_eq!(bytes.len(), sent);
        assert!(bytes.windows(7).any(|w| w == b"Standup"));
        assert!(
            bytes.windows(11).any(|w| w == b"08 SEP 2026"),
            "dated the run day"
        );

        let gone = run(&printers, &spec("nope", "* * * * *", None), at(8, 9))
            .await
            .unwrap();
        assert_eq!(gone, 0);
    }
}
