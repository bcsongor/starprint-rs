import { differenceInCalendarDays, format, parseISO } from "date-fns";
import {
  TWO_COLOR_DENSITY,
  type Job,
  type Rule,
  type Scheduled,
} from "@/lib/api";
import { toPrinter, type Profile, type Schedule } from "@/lib/settings";

/** The common timetables, each as the day fields of its expression. */
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

/** "Mon 09:00" within the week, "5 Oct 09:00" beyond it. */
export function formatSoon(iso: string): string {
  const at = parseISO(iso);
  const pattern =
    differenceInCalendarDays(at, new Date()) < 7 ? "EEE HH:mm" : "d MMM HH:mm";
  return format(at, pattern);
}

/** "Mon 8 Sep, 09:00". */
export function formatNext(iso: string): string {
  return format(parseISO(iso), "EEE d MMM, HH:mm");
}

/** Names a job kind in a toast or a title. */
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
      return job.path.split(/[\\/]/).pop() ?? job.path;
    case "test-page":
      return "Test page";
  }
}

/**
 * A schedule as the Rust side runs it, or null once its profile is gone.
 * A picture loses double resolution on a two-colour profile, which has
 * none, as it does in the form.
 */
export function toScheduled(
  schedule: Schedule,
  profiles: Profile[],
): Scheduled | null {
  const profile = profiles.find((p) => p.id === schedule.profileId);
  if (!profile) return null;
  const printer = toPrinter(profile);
  const { id, job, cron, due } = schedule;
  const twoColor =
    printer.kind === "thermal" && printer.density === TWO_COLOR_DENSITY;
  return {
    id,
    job: job.kind === "picture" && twoColor ? { ...job, double: false } : job,
    printer,
    cron,
    due,
  };
}

/** A schedule for `job` on `profileId`, every weekday at nine. A task
 * card is dated the day it prints. */
export function newSchedule(job: Job, profileId: string): Schedule {
  return {
    id: crypto.randomUUID(),
    job,
    profileId,
    cron: toCron("weekday", "09:00"),
    due: job.kind === "task-card" ? "run-day" : null,
    enabled: true,
  };
}
