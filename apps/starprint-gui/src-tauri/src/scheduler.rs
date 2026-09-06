//! Runs the frontend's schedules through the shared print queue.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Days, Local, Timelike};
use croner::Cron;
use croner::errors::CronError;
use croner::parser::{CronParser, Seconds};
use serde::{Deserialize, Serialize};
use starprint_api::PrintQueue;
use starprint_workflows::{Job, Printer};
use tauri::{AppHandle, Emitter, Manager};

use crate::job::{self, JobRequest, PrintReport};
use crate::picture::SourceCache;

const RAN_EVENT: &str = "schedule-ran";

fn parse_cron(expression: &str) -> Result<Cron, CronError> {
    CronParser::builder()
        .seconds(Seconds::Disallowed)
        .build()
        .parse(expression)
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Due {
    RunDay,
    NextDay,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scheduled {
    id: String,
    job: JobRequest,
    printer: Printer,
    cron: String,
    due: Option<Due>,
}

impl Scheduled {
    fn job_at(&self, at: DateTime<Local>) -> JobRequest {
        let mut request = self.job.clone();
        if let Job::TaskCard(card) = &mut request.job {
            card.due = self.due.map(|due| {
                let days = match due {
                    Due::RunDay => 0,
                    Due::NextDay => 1,
                };
                let date = at.date_naive() + Days::new(days);
                date.format("%Y-%m-%d").to_string()
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

#[derive(Clone, Serialize)]
struct Outcome {
    id: String,
    error: Option<String>,
}

#[derive(Default)]
pub struct Scheduler(Mutex<Vec<(Scheduled, Cron)>>);

impl Scheduler {
    fn replace(&self, schedules: Vec<Scheduled>) -> Result<(), String> {
        let entries = schedules
            .into_iter()
            .filter(|scheduled| !scheduled.printer.host.trim().is_empty())
            .map(|scheduled| {
                let cron = parse_cron(&scheduled.cron).map_err(|e| e.to_string())?;
                Ok((scheduled, cron))
            })
            .collect::<Result<_, String>>()?;
        *self.0.lock().unwrap() = entries;
        Ok(())
    }

    fn matching(&self, now: DateTime<Local>) -> Vec<Scheduled> {
        let minute = now.with_second(0).unwrap();
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cron)| cron.is_time_matching(&minute).unwrap())
            .map(|(scheduled, _)| scheduled.clone())
            .collect()
    }
}

#[tauri::command]
pub fn set_schedules(
    schedules: Vec<Scheduled>,
    scheduler: tauri::State<'_, Scheduler>,
) -> Result<(), String> {
    scheduler.replace(schedules)
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let wait = 60_000 - Local::now().timestamp_millis().rem_euclid(60_000);
            tokio::time::sleep(Duration::from_millis(wait as u64)).await;
            for scheduled in app.state::<Scheduler>().matching(Local::now()) {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let cache = app.state::<Arc<SourceCache>>();
                    let queue = app.state::<Arc<PrintQueue>>();
                    let error = scheduled.print(&cache, &queue).await.err();
                    let _ = app.emit(
                        RAN_EVENT,
                        Outcome {
                            id: scheduled.id,
                            error,
                        },
                    );
                });
            }
        }
    });
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
    let cron = parse_cron(&cron).map_err(|e| e.to_string())?;
    cron.find_next_occurrence(&Local::now(), false)
        .map_err(|e| e.to_string())
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
    fn schedules_match_the_current_minute() {
        let scheduler = Scheduler::default();
        let mut no_host = scheduled("no-host", "* * * * *", None);
        no_host.printer.host.clear();
        scheduler
            .replace(vec![scheduled("a", "0 9 * * *", None), no_host])
            .unwrap();
        assert!(scheduler.matching(at(8, 8)).is_empty());
        assert_eq!(
            scheduler.matching(at(8, 9).with_second(17).unwrap())[0].id,
            "a"
        );
        assert!(scheduler.matching(at(8, 10)).is_empty());
        scheduler
            .replace(vec![scheduled("b", "0 10 * * *", None)])
            .unwrap();
        assert!(scheduler.matching(at(8, 9)).is_empty());
        assert_eq!(scheduler.matching(at(8, 10))[0].id, "b");
        scheduler.replace(Vec::new()).unwrap();
        assert!(scheduler.matching(at(8, 10)).is_empty());
    }

    #[test]
    fn expressions_with_fewer_or_more_than_five_fields_are_refused() {
        for cron in ["0 9 * *", "*/5 * * * * *", "0 0 9 * * * 2026"] {
            assert!(next_run(cron.to_owned()).is_err(), "{cron}");
        }
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
