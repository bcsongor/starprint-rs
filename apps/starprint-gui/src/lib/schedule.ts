import { Cron } from "croner";
import { format } from "date-fns";
import type { Job, Rule, ScheduleSpec } from "@/lib/api";

export const PRESETS = {
  weekday: {
    label: "Every weekday",
    days: "* * 1-5",
    readout: "Monday to Friday",
  },
  daily: { label: "Every day", days: "* * *", readout: "every day" },
  monday: { label: "Every Monday", days: "* * 1", readout: "on Monday" },
  "first-monday": {
    label: "First Monday of the month",
    days: "* * 1#1",
    readout: "on the first Monday of the month",
  },
};

export type Preset = keyof typeof PRESETS;

/** A preset at `HH:MM` as a cron expression. */
export function toCron(preset: Preset, time: string): string {
  const [hour, minute] = time.split(":").map(Number);
  return `${minute} ${hour} ${PRESETS[preset].days}`;
}

/** The preset and time an expression spells, if it spells one. */
export function fromCron(
  cron: string,
): { preset: Preset; time: string } | null {
  const [minute, hour, ...rest] = cron.trim().split(/\s+/);
  const days = rest.join(" ");
  const preset = (Object.keys(PRESETS) as Preset[]).find(
    (p) => PRESETS[p].days === days,
  );
  if (!preset || !/^\d{1,2}$/.test(minute) || !/^\d{1,2}$/.test(hour)) {
    return null;
  }
  const time = `${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`;
  return { preset, time };
}

/** "At 09:00, Monday to Friday" for a preset; otherwise the expression. */
export function describe(cron: string): string {
  const match = fromCron(cron);
  if (!match) return cron;
  return `At ${match.time}, ${PRESETS[match.preset].readout}`;
}

/**
 * When `cron` next fires, or throws with what is wrong with it. Five
 * fields, as the server takes them; the dialect is the same, so what
 * passes here is what the server accepts.
 */
export function nextRun(cron: string): Date {
  const next = new Cron(cron, { mode: "5-part" }).nextRun();
  if (!next) throw new Error("This never fires.");
  return next;
}

/** "Mon 8 Sep, 09:00". */
export function formatNext(at: Date): string {
  return format(at, "EEE d MMM, HH:mm");
}

export const NAMES: Record<Job["kind"], string> = {
  "task-card": "task card",
  text: "text",
  note: "note slip",
  qr: "QR code",
  picture: "picture",
  "test-page": "test page",
};

const RULES: Record<Rule, string> = {
  blank: "Blank",
  dots: "Dotted",
  lines: "Lined",
  squares: "Squared",
};

/** A job in a line, for a schedule's row and its dialog. */
export function summary(job: Job): string {
  switch (job.kind) {
    case "task-card":
    case "text":
      return job.text.trim().split("\n")[0];
    case "note":
      return `${RULES[job.rule]} note, ${job.rows} rows`;
    case "qr":
      return job.data;
    case "picture":
      return "Picture";
    case "test-page":
      return "Test page";
  }
}

/** A schedule for `job` on `printer`, every weekday at nine. A task
 * card is dated the day it prints. */
export function newSchedule(job: Job, printer: string): ScheduleSpec {
  return {
    printer,
    cron: toCron("weekday", "09:00"),
    enabled: true,
    due: job.kind === "task-card" ? "run-day" : undefined,
    job,
  };
}
