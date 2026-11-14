/**
 * The client of `starprint-api`. Types mirror `apps/starprint-api/API.md`;
 * the job and preview shapes are those of `starprint-workflows`.
 */

export type PrinterKind = "thermal" | "impact";

export type Speed = "high" | "medium" | "slow";

/** Roll width in millimetres, matching the print width memory switch. */
type Paper = 80 | 112;

/** Selects two-colour mode; mirrors `TWO_COLOR_DENSITY` in starprint-workflows. */
export const TWO_COLOR_DENSITY = 4;

/**
 * A profile as `PUT /v1/printers/{name}` takes it. Only a thermal
 * printer has a roll width, a density and a speed.
 */
export type ProfileSpec = {
  host: string;
  port: number;
  cut: boolean;
  /** Free text nothing reads: firmware, where the printer sits. */
  notes: string;
} & (
  | { kind: "thermal"; paper: Paper; density: number; speed: Speed }
  | { kind: "impact" }
);

/** A profile as the server lists it. */
export type Profile = ProfileSpec & { name: string };

export function toSpec({ name: _name, ...spec }: Profile): ProfileSpec {
  return spec;
}

/** What a thermal profile starts with, new or switched from impact. */
export const THERMAL_DEFAULTS = {
  kind: "thermal",
  paper: 80,
  density: 3,
  speed: "slow",
} satisfies Partial<ProfileSpec>;

/** The profiles as the picker lists them, in alphabetical order. */
export function byName(profiles: Profile[]): Profile[] {
  return [...profiles].sort((a, b) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" }),
  );
}

export interface TaskCard {
  text: string;
  priority: boolean;
  /** An issue key from Linear; the form has no box for it. */
  reference: string | null;
  /** ISO date or free text. */
  due: string | null;
}

export interface TaskCardLayout {
  columns: number;
  priority: string | null;
  /** Padded to sit centrally between the banner and the date. */
  reference: string | null;
  due: string | null;
  lines: string[];
}

/** A stretch of text in one style. A style that is off may be left out. */
export interface Run {
  text: string;
  bold?: boolean;
  /** Double width. */
  wide?: boolean;
  /** Double height. */
  tall?: boolean;
  /** Red on impact, inverse on thermal. */
  accent?: boolean;
}

export type Style = Exclude<keyof Run, "text">;

/** Text whose style can change partway, as runs in order. A line
 * break is a `\n` in a run. */
export interface Text {
  runs: Run[];
}

export interface TextLayout {
  /** At normal size, so a wide character takes two. */
  columns: number;
  /** Each line's characters, whatever their styles. */
  lines: string[];
  /** Each line as its runs. A blank line is one empty run, in the
   * style its line feed prints in. */
  runs: Run[][];
}

export const DEFAULT_TEXT: Text = { runs: [] };

export type Rule = "blank" | "dots" | "lines" | "squares";

export interface Note {
  rule: Rule;
  /** Rows to write in. */
  rows: number;
  /** Millimetres between the rules, across and down. */
  pitch: number;
}

export interface NoteLayout {
  columns: number;
  /** Right-aligned to fill the header line. */
  date: string;
}

/**
 * The pitch ruled paper is sold at: 7 mm between lines, 5 mm squares
 * and a 5 mm dot grid. Blank keeps the lined pitch, since there it only
 * decides how long the slip is.
 *
 * The rows go with the pitch, so every ruling tears off at the same
 * 70 mm as the 10 lines of 7 mm this is measured from.
 */
export const NOTEBOOK_RULING: Record<Rule, { rows: number; pitch: number }> = {
  blank: { rows: 10, pitch: 7 },
  dots: { rows: 14, pitch: 5 },
  lines: { rows: 10, pitch: 7 },
  squares: { rows: 14, pitch: 5 },
};

export const DEFAULT_NOTE: Note = { rule: "lines", ...NOTEBOOK_RULING.lines };

export type Align = "left" | "center" | "right";

export type Ecc = "l" | "m" | "q" | "h";

/** How much of a symbol each level can lose and still scan. */
export const RECOVERY: Record<Ecc, string> = {
  l: "7%",
  m: "15%",
  q: "25%",
  h: "30%",
};

export interface Qr {
  data: string;
  /** Printed above the symbol. */
  caption: string | null;
  errorCorrection: Ecc;
  /** The symbol's width in millimetres, quiet zone excluded. */
  size: number;
  /**
   * How far the corners of the symbol are rounded: 0 leaves every one
   * square, 100 rounds each as far as its shape allows. A lone module
   * is a circle a third of the way along; larger blocks keep going.
   */
  radius: number;
  /** Carries the caption with it. */
  align: Align;
}

/** `MIN_SIZE_MM` and `MAX_SIZE_MM` in starprint-workflows/src/qr.rs. */
export const MIN_QR_MM = 10;
export const MAX_QR_MM = 80;

/** `MAX_RADIUS` in the same file; the floor is a square module. */
export const MAX_QR_RADIUS = 100;

export const DEFAULT_QR: Qr = {
  data: "",
  caption: null,
  errorCorrection: "m",
  size: 30,
  radius: 0,
  align: "center",
};

export interface QrLayout {
  columns: number;
  /** Wrapped to the paper; empty when there is no caption. */
  caption: string[];
  align: Align;
  /** Modules across the block, quiet zone included. */
  modules: number;
  /** What the block measures on paper, quiet zone included. */
  widthMm: number;
  heightMm: number;
}

/** The quiet zone each side; `QUIET_MODULES` in the same file. */
export const QUIET_MODULES = 4;

export interface TestPage {
  doubleResolution: boolean;
}

export const DEFAULT_TEST_PAGE: TestPage = { doubleResolution: false };

export interface Section {
  title: string;
  check: string;
}

export type Dither = "floyd-steinberg" | "atkinson" | "threshold" | "bayer";

/** The settings; the image itself goes beside the job as a form part. */
export interface Picture {
  /** Impact: double density. Thermal: double-resolution mode. */
  double: boolean;
  dither: Dither;
  /** Ignored by Bayer. */
  threshold: number;
  brightness: number;
  contrast: number;
}

export const DEFAULT_PICTURE: Picture = {
  double: false,
  dither: "floyd-steinberg",
  threshold: 128,
  brightness: 1,
  contrast: 1,
};

export type Job =
  | ({ kind: "task-card" } & TaskCard)
  | ({ kind: "text" } & Text)
  | ({ kind: "note" } & Note)
  | ({ kind: "qr" } & Qr)
  | ({ kind: "test-page" } & TestPage)
  | ({ kind: "picture" } & Picture);

/** The job as it will look, tagged by `kind` like the job. */
export type Preview =
  | ({ kind: "task-card" } & TaskCardLayout)
  | ({ kind: "text" } & TextLayout)
  | ({ kind: "note"; image: string } & NoteLayout)
  | ({ kind: "qr"; image: string } & QrLayout)
  | { kind: "test-page"; sections: Section[] }
  | { kind: "picture"; image: string };

export interface PrintReport {
  /** A completed socket write, and nothing more. */
  bytesSent: number;
}

/** The due date a scheduled task card gets when it prints. */
export type Due = "run-day" | "next-day";

/** A schedule as `POST /v1/schedules` takes it. */
export interface ScheduleSpec {
  printer: string;
  /** Five fields: minute, hour, day, month, weekday. */
  cron: string;
  enabled: boolean;
  /** Task cards only; absent prints the card undated. */
  due?: Due;
  /** Not a picture: the server holds no image to print one from. */
  job: Exclude<Job, { kind: "picture" }>;
}

export interface Schedule extends ScheduleSpec {
  id: string;
}

/** A text job in the plain shape, a run's fields beside `kind`, which
 * every server reads. */
type PlainText = { kind: "text" } & Run;

/** A job as the server writes it, and as the app sends it so that a
 * server from before runs reads it too: a text job of one run in the
 * plain shape, and one of several with their characters as `text`
 * beside `runs`, which such a server prints without the styles. */
type JobWire =
  | Exclude<Job, { kind: "text" }>
  | PlainText
  | { kind: "text"; runs: Run[]; text: string };

function writeJob(job: Job): JobWire {
  if (job.kind !== "text") return job;
  const { runs } = job;
  if (runs.length <= 1) return { kind: "text", ...(runs[0] ?? { text: "" }) };
  return { kind: "text", runs, text: runs.map((run) => run.text).join("") };
}

/** A schedule as the server writes it. */
type ScheduleWire = Omit<Schedule, "job"> & {
  job: Schedule["job"] | PlainText;
};

function writeSchedule({ job, ...spec }: ScheduleSpec) {
  return { ...spec, job: writeJob(job) };
}

/** `schedule` with its job as the app keeps it, a text job as runs. */
function readSchedule({ job, ...schedule }: ScheduleWire): Schedule {
  if (job.kind !== "text") return { ...schedule, job };
  if ("runs" in job)
    return { ...schedule, job: { kind: "text", runs: job.runs } };
  const { kind, ...run } = job;
  return { ...schedule, job: { kind, runs: [run] } };
}

/** A text layout from a server before runs: its lines as strings. */
type OlderTextPreview = { kind: "text"; columns: number; lines: string[] };

/** `preview` with a text layout's lines as runs. A server from before
 * runs prints a job of one run in that run's style and the runs of
 * several without styles, so its lines are drawn that way. */
function readPreview(preview: Preview | OlderTextPreview, job: Job): Preview {
  if (preview.kind !== "text" || "runs" in preview) return preview;
  const style = job.kind === "text" && job.runs.length === 1 ? job.runs[0] : {};
  return {
    ...preview,
    runs: preview.lines.map((text) => [{ ...style, text }]),
  };
}

export interface Server {
  url: string;
  token: string;
}

/** A relay as the server knows it, by the name it gave. */
export interface FaxRelay {
  name: string;
  url: string;
  /** Null until the server has polled it. */
  online: boolean | null;
  /** Why the last poll failed. */
  problem?: string;
}

/** A number in the fax book, under the name given here. */
export interface FaxContact {
  name: string;
  number: string;
}

/** A number this line faxed or printed a fax from lately. */
export interface FaxRecent {
  number: string;
  /** Unix seconds. */
  at: number;
}

/** The server's fax line, as `GET /v1/fax` gives it. */
export interface FaxLine {
  /** Like `*star1en2su3z68yscvky0n3j3l2qwny4dkq7s`; null until the line is activated. */
  number: string | null;
  /** The profile faxes print on; null for the first one. */
  printer: string | null;
  relays: FaxRelay[];
  /** The fax book, by name. */
  contacts: FaxContact[];
  /** Those not in the fax book, newest first. */
  recent: FaxRecent[];
}

export interface Sent {
  to: string;
  /** The recipient's name in the fax book, if they are in it. */
  name?: string;
  relay: string;
}

/** A job, as JSON or, with an image beside it, as a form. */
function jobBody(job: Job, image: File | null): RequestInit {
  return requestBody({ job: writeJob(job) }, image);
}

/** A request that carries a job, with its picture as a form part. */
function requestBody(value: object, image: File | null): RequestInit {
  if (!image) return jsonBody(value);
  const form = new FormData();
  form.append("job", JSON.stringify(value));
  form.append("image", image);
  return { body: form };
}

function jsonBody(value: unknown): RequestInit {
  return {
    body: JSON.stringify(value),
    headers: { "Content-Type": "application/json" },
  };
}

/** A problem's `detail`, if `body` is a problem at all. */
function detail(body: unknown): string | undefined {
  if (typeof body !== "object" || body === null || !("detail" in body)) {
    return undefined;
  }
  return typeof body.detail === "string" ? body.detail : undefined;
}

export function createClient({ url, token }: Server) {
  /** The response, or the problem's `detail` thrown. */
  const send = async (method: string, path: string, init: RequestInit = {}) => {
    const response = await fetch(url + path, {
      ...init,
      method,
      headers: { ...init.headers, Authorization: `Bearer ${token}` },
    });
    if (!response.ok) {
      // A proxy, or an address that is not this server's, answers with
      // something other than a problem, so the status has to do.
      const body: unknown = await response.json().catch(() => null);
      throw new Error(
        detail(body) ?? (response.statusText || `HTTP ${response.status}`),
      );
    }
    return response;
  };
  const json = <T>(method: string, path: string, init?: RequestInit) =>
    send(method, path, init).then((response) => response.json() as Promise<T>);
  const printer = (name: string) => `/v1/printers/${encodeURIComponent(name)}`;
  /** A number in a path goes without its star. */
  const contact = (number: string) =>
    `/v1/fax/contacts/${encodeURIComponent(number.trim().replace(/^\*/, ""))}`;

  return {
    printers: () =>
      json<{ version: string; printers: Profile[] }>("GET", "/v1/printers"),
    putPrinter: (name: string, spec: ProfileSpec) =>
      json<Profile>("PUT", printer(name), jsonBody(spec)),
    deletePrinter: (name: string) => send("DELETE", printer(name)),
    status: (name: string) =>
      json<{ online: boolean }>("GET", `${printer(name)}/status`).then(
        (status) => status.online,
      ),
    print: (name: string, job: Job, image: File | null) =>
      json<PrintReport>("POST", `${printer(name)}/jobs`, jobBody(job, image)),
    preview: (name: string, job: Job, image: File | null) =>
      json<Preview | OlderTextPreview>(
        "POST",
        `${printer(name)}/preview`,
        jobBody(job, image),
      ).then((preview) => readPreview(preview, job)),
    schedules: () =>
      json<ScheduleWire[]>("GET", "/v1/schedules").then((list) =>
        list.map(readSchedule),
      ),
    createSchedule: (spec: ScheduleSpec) =>
      json<ScheduleWire>(
        "POST",
        "/v1/schedules",
        jsonBody(writeSchedule(spec)),
      ).then(readSchedule),
    replaceSchedule: (id: string, spec: ScheduleSpec) =>
      json<ScheduleWire>(
        "PUT",
        `/v1/schedules/${id}`,
        jsonBody(writeSchedule(spec)),
      ).then(readSchedule),
    deleteSchedule: (id: string) => send("DELETE", `/v1/schedules/${id}`),
    fax: () => json<FaxLine>("GET", "/v1/fax"),
    putFax: (settings: { printer: string | null }) =>
      json<FaxLine>("PUT", "/v1/fax", jsonBody(settings)),
    activateFax: () => json<FaxLine>("POST", "/v1/fax/line"),
    replaceFaxLine: () => json<FaxLine>("PUT", "/v1/fax/line"),
    addRelay: (url: string) =>
      json<{ name: string; url: string }>(
        "POST",
        "/v1/fax/relays",
        jsonBody({ url }),
      ),
    removeRelay: (name: string) =>
      send("DELETE", `/v1/fax/relays/${encodeURIComponent(name)}`),
    putContact: (number: string, name: string) =>
      json<FaxContact>("PUT", contact(number), jsonBody({ name })),
    deleteContact: (number: string) => send("DELETE", contact(number)),
    sendFax: (to: string, job: Job, image: File | null) =>
      json<Sent>(
        "POST",
        "/v1/fax/send",
        requestBody({ to, job: writeJob(job) }, image),
      ),
  };
}

export type Client = ReturnType<typeof createClient>;
