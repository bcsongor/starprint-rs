//! Runs the frontend's schedules through the shared print queue.
//! Persists run times so missed occurrences print once at the next start.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Days, Local};
use croner::Cron;
use serde::{Deserialize, Serialize};
use starprint_api::PrintQueue;
use starprint_workflows::{Job, Printer};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::Notify;

use crate::job::{self, JobRequest, PrintReport};
use crate::picture::SourceCache;

/// The frontend's settings file. Only `RUNS_KEY` is written from here.
const STORE: &str = "settings.json";
const RUNS_KEY: &str = "scheduleRuns";
const RAN_EVENT: &str = "schedule-ran";
/// The longest the loop sleeps at once. Tokio's timer does not count
/// time spent suspended on every platform, so a machine back from sleep
/// looks at the clock again within this.
const MAX_SLEEP: Duration = Duration::from_secs(60);

/// The due date a scheduled task card gets when it prints.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Due {
    RunDay,
    NextDay,
}

/// A schedule as the frontend sends it: the job as its form stood, the
/// printer its profile resolves to, and a five-field cron expression.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scheduled {
    id: String,
    job: JobRequest,
    printer: Printer,
    cron: String,
    #[serde(default)]
    due: Option<Due>,
}

impl Scheduled {
    /// The job for a run at `at`. A task card's due date comes from the
    /// rule, not from whatever its form held when it was scheduled.
    fn job_at(&self, at: DateTime<Local>) -> JobRequest {
        let mut request = self.job.clone();
        if let Job::TaskCard(card) = &mut request.job {
            card.due = self.due.and_then(|due| {
                let days = match due {
                    Due::RunDay => 0,
                    Due::NextDay => 1,
                };
                let date = at.date_naive().checked_add_days(Days::new(days))?;
                Some(date.format("%Y-%m-%d").to_string())
            });
        }
        request
    }

    /// Prints the job, dated the day its turn on the printer comes.
    async fn print(
        &self,
        cache: &Arc<SourceCache>,
        queue: &PrintQueue,
    ) -> Result<PrintReport, String> {
        let turn = queue.lock(&self.printer.host, self.printer.port).await;
        let job = self.job_at(Local::now());
        job::print_on(turn, job, self.printer.clone(), Arc::clone(cache)).await
    }
}

/// When a schedule last ran, kept in the store under its id. The
/// expression sits beside it so a changed timetable starts afresh.
#[derive(Serialize, Deserialize)]
struct Run {
    at: DateTime<Local>,
    cron: String,
}

struct Entry {
    scheduled: Scheduled,
    cron: Cron,
    /// The next run is the first occurrence after this.
    last_run: DateTime<Local>,
}

impl Entry {
    fn next(&self) -> Option<DateTime<Local>> {
        self.cron.find_next_occurrence(&self.last_run, false).ok()
    }
}

/// What became of a scheduled print, for the frontend to report.
#[derive(Clone, Serialize)]
struct Outcome {
    id: String,
    error: Option<String>,
}

#[derive(Default)]
pub struct Scheduler {
    entries: Mutex<Vec<Entry>>,
    changed: Notify,
}

impl Scheduler {
    /// Replaces the entries. A schedule keeps its last run while its id
    /// and expression stay the same, the live one over the stored one so
    /// a print still in progress is not repeated; anything else starts
    /// from `now`, so nothing prints for the time before it was scheduled.
    fn replace(
        &self,
        schedules: Vec<Scheduled>,
        stored: &HashMap<String, Run>,
        now: DateTime<Local>,
    ) -> Result<(), String> {
        let live = self.runs();
        let entries = schedules
            .into_iter()
            .map(|scheduled| {
                let cron = Cron::from_str(&scheduled.cron)
                    .map_err(|e| format!("{}: {e}", scheduled.cron))?;
                let last_run = live
                    .get(&scheduled.id)
                    .or_else(|| stored.get(&scheduled.id))
                    .filter(|run| run.cron == scheduled.cron)
                    .map_or(now, |run| run.at);
                Ok(Entry {
                    scheduled,
                    cron,
                    last_run,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        *self.entries.lock().unwrap() = entries;
        self.changed.notify_one();
        Ok(())
    }

    /// One schedule due by `now`, marked as run at `now`, so a backlog of
    /// missed occurrences collapses into one print. One at a time, so a
    /// schedule removed while another prints does not print after it.
    fn take_due(&self, now: DateTime<Local>) -> Option<Scheduled> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries
            .iter_mut()
            .find(|entry| entry.next().is_some_and(|next| next <= now))?;
        entry.last_run = now;
        Some(entry.scheduled.clone())
    }

    /// Moves a schedule's last run to `at` once its print has finished,
    /// so the next occurrence counts from the end of a long print rather
    /// than following it at once.
    fn ran(&self, id: &str, at: DateTime<Local>) {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter_mut().find(|entry| entry.scheduled.id == id) {
            entry.last_run = at;
        }
    }

    /// How long until the earliest next run, if there is one.
    fn until_next(&self, now: DateTime<Local>) -> Option<Duration> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter_map(Entry::next)
            .min()
            .map(|next| (next - now).to_std().unwrap_or_default())
    }

    fn runs(&self) -> HashMap<String, Run> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .map(|entry| {
                let run = Run {
                    at: entry.last_run,
                    cron: entry.scheduled.cron.clone(),
                };
                (entry.scheduled.id.clone(), run)
            })
            .collect()
    }
}

fn persist(app: &AppHandle, scheduler: &Scheduler) -> Result<(), String> {
    let runs = serde_json::to_value(scheduler.runs()).map_err(|e| e.to_string())?;
    let store = app.store(STORE).map_err(|e| e.to_string())?;
    store.set(RUNS_KEY, runs);
    store.save().map_err(|e| e.to_string())
}

/// Replaces the schedules to run. Rejects the first bad expression.
#[tauri::command]
pub fn set_schedules(
    schedules: Vec<Scheduled>,
    app: AppHandle,
    scheduler: tauri::State<'_, Scheduler>,
) -> Result<(), String> {
    let runs = app
        .store(STORE)
        .map_err(|e| e.to_string())?
        .get(RUNS_KEY)
        .and_then(|runs| serde_json::from_value(runs).ok())
        .unwrap_or_default();
    scheduler.replace(schedules, &runs, Local::now())?;
    persist(&app, &scheduler)
}

/// Prints a schedule's job now, as a run would.
#[tauri::command]
pub async fn print_scheduled(
    scheduled: Scheduled,
    cache: tauri::State<'_, Arc<SourceCache>>,
    queue: tauri::State<'_, Arc<PrintQueue>>,
) -> Result<PrintReport, String> {
    scheduled.print(&cache, &queue).await
}

/// When `cron` next fires, or what is wrong with it.
#[tauri::command]
pub fn next_run(cron: String) -> Result<DateTime<Local>, String> {
    let cron = Cron::from_str(&cron).map_err(|e| e.to_string())?;
    cron.find_next_occurrence(&Local::now(), false)
        .map_err(|e| e.to_string())
}

/// Runs the schedules until the app exits.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let scheduler = app.state::<Scheduler>();
        let cache = Arc::clone(&app.state::<Arc<SourceCache>>());
        let queue = Arc::clone(&app.state::<Arc<PrintQueue>>());
        let record = |scheduler: &Scheduler| {
            if let Err(e) = persist(&app, scheduler) {
                eprintln!("could not record schedule runs: {e}");
            }
        };
        loop {
            while let Some(scheduled) = scheduler.take_due(Local::now()) {
                // Recorded before the print, so an exit during it does not
                // repeat the run at the next start.
                record(&scheduler);
                let error = scheduled.print(&cache, &queue).await.err();
                scheduler.ran(&scheduled.id, Local::now());
                record(&scheduler);
                let outcome = Outcome {
                    id: scheduled.id,
                    error,
                };
                let _ = app.emit(RAN_EVENT, outcome);
            }
            let wait = scheduler
                .until_next(Local::now())
                .map_or(MAX_SLEEP, |until| until.min(MAX_SLEEP));
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = scheduler.changed.notified() => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn scheduled(id: &str, cron: &str, due: Option<Due>) -> Scheduled {
        let json = r#"{"kind":"task-card","text":"Standup","due":"2026-01-01"}"#;
        let job = serde_json::from_str(json).unwrap();
        Scheduled {
            id: id.to_owned(),
            job,
            printer: Printer::impact("printer.invalid".to_owned(), 9100),
            cron: cron.to_owned(),
            due,
        }
    }

    fn at(day: u32, hour: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 9, day, hour, 0, 0).unwrap()
    }

    #[test]
    fn a_missed_run_prints_once_and_the_next_waits_for_its_time() {
        let scheduler = Scheduler::default();
        let runs = HashMap::from([(
            "a".to_owned(),
            Run {
                at: at(3, 9),
                cron: "0 9 * * *".to_owned(),
            },
        )]);
        scheduler
            .replace(vec![scheduled("a", "0 9 * * *", None)], &runs, at(6, 12))
            .unwrap();

        assert!(
            scheduler.take_due(at(6, 12)).is_some(),
            "three mornings missed"
        );
        assert!(scheduler.take_due(at(6, 12)).is_none());
        assert_eq!(
            scheduler.until_next(at(6, 12)),
            Some(Duration::from_secs(21 * 3600)),
            "tomorrow at nine"
        );
    }

    #[test]
    fn a_new_or_changed_schedule_starts_from_now() {
        let scheduler = Scheduler::default();
        let runs = HashMap::from([(
            "a".to_owned(),
            Run {
                at: at(3, 9),
                cron: "0 9 * * *".to_owned(),
            },
        )]);
        scheduler
            .replace(
                vec![
                    scheduled("a", "0 10 * * *", None),
                    scheduled("b", "0 9 * * *", None),
                ],
                &runs,
                at(6, 12),
            )
            .unwrap();
        assert!(scheduler.take_due(at(6, 12)).is_none());
    }

    #[test]
    fn a_run_in_progress_survives_the_schedules_being_replaced() {
        let scheduler = Scheduler::default();
        let stored = HashMap::from([(
            "a".to_owned(),
            Run {
                at: at(5, 9),
                cron: "0 9 * * *".to_owned(),
            },
        )]);
        scheduler
            .replace(vec![scheduled("a", "0 9 * * *", None)], &stored, at(6, 12))
            .unwrap();
        assert!(scheduler.take_due(at(6, 12)).is_some());

        // The store still says yesterday until the run is persisted.
        scheduler
            .replace(
                vec![
                    scheduled("a", "0 9 * * *", None),
                    scheduled("b", "0 9 * * *", None),
                ],
                &stored,
                at(6, 12),
            )
            .unwrap();
        assert!(scheduler.take_due(at(6, 12)).is_none());
    }

    #[test]
    fn a_print_longer_than_its_interval_is_followed_by_a_gap() {
        let scheduler = Scheduler::default();
        scheduler
            .replace(
                vec![scheduled("a", "* * * * *", None)],
                &HashMap::new(),
                at(6, 9),
            )
            .unwrap();
        assert!(
            scheduler.take_due(at(6, 9)).is_none(),
            "nothing before the first minute"
        );
        assert!(scheduler.take_due(at(6, 10)).is_some());

        let finished = Local.with_ymd_and_hms(2026, 9, 6, 10, 1, 30).unwrap();
        scheduler.ran("a", finished);
        assert!(scheduler.take_due(finished).is_none());
        assert_eq!(
            scheduler.until_next(finished),
            Some(Duration::from_secs(30)),
            "the minute after the print ends"
        );
    }

    #[test]
    fn a_bad_expression_is_refused_by_name() {
        let error = Scheduler::default()
            .replace(
                vec![scheduled("a", "0 9 * *", None)],
                &HashMap::new(),
                at(6, 12),
            )
            .unwrap_err();
        assert!(error.starts_with("0 9 * *: "), "{error}");
    }

    #[test]
    fn a_task_card_is_dated_when_it_runs() {
        let due = |rule| {
            let Job::TaskCard(card) = scheduled("a", "0 9 * * *", rule).job_at(at(8, 9)).job else {
                panic!("task card")
            };
            card.due
        };
        assert_eq!(due(None), None, "the form's date does not carry over");
        assert_eq!(due(Some(Due::RunDay)).as_deref(), Some("2026-09-08"));
        assert_eq!(due(Some(Due::NextDay)).as_deref(), Some("2026-09-09"));
    }
}
