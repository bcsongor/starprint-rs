import type { Job, Preview, Rule, Text } from "@/lib/api";

export type Kind = Job["kind"];

/** The job of each kind, and the preview the server answers it with. */
export type Jobs = { [K in Kind]: Extract<Job, { kind: K }> };
export type Previews = { [K in Kind]: Extract<Preview, { kind: K }> };

export const NAMES: Record<Kind, string> = {
  "task-card": "task card",
  text: "text",
  note: "note slip",
  qr: "QR code",
  picture: "picture",
  "test-page": "test page",
};

/** Whether `value` names a kind of job, for one that arrives untyped. */
export function isKind(value: unknown): value is Kind {
  return typeof value === "string" && Object.hasOwn(NAMES, value);
}

const RULES: Record<Rule, string> = {
  blank: "Blank",
  dots: "Dotted",
  lines: "Lined",
  squares: "Squared",
};

/** The characters of `text`, whatever their styles. */
export function plain(text: Text): string {
  return text.runs.map((run) => run.text).join("");
}

/** A job in a line, for a schedule's row and its dialog. */
export function summary(job: Job): string {
  switch (job.kind) {
    case "task-card":
      return job.text.trim().split("\n")[0];
    case "text":
      return plain(job).trim().split("\n")[0];
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

/** Whether the form holds a job worth sending; a picture needs its `image`. */
export function isReady(job: Job, image: File | null): boolean {
  switch (job.kind) {
    case "task-card":
      return job.text.trim().length > 0;
    case "text":
      return plain(job).trim().length > 0;
    case "qr":
      return job.data.trim().length > 0;
    case "picture":
      return image !== null;
    case "note":
    case "test-page":
      return true;
  }
}

/**
 * Whether there is anything to preview. Empty text still previews, as
 * a placeholder; an empty QR code or a missing picture has nothing to
 * show yet.
 */
export function isPreviewable(job: Job, image: File | null): boolean {
  return job.kind === "qr" || job.kind === "picture"
    ? isReady(job, image)
    : true;
}
