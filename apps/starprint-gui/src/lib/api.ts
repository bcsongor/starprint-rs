/**
 * The client of `starprint-api`. Types mirror `apps/starprint-api/API.md`;
 * the job and preview shapes are those of `starprint-workflows`.
 */

export type PrinterKind = "thermal" | "impact";

export type Speed = "high" | "medium" | "slow";

/** Roll width in millimetres, matching the print width memory switch. */
export type Paper = 80 | 112;

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
} & (
  | { kind: "thermal"; paper: Paper; density: number; speed: Speed }
  | { kind: "impact" }
);

/** A profile as the server lists it. */
export type Profile = ProfileSpec & { name: string };

export function toSpec({ name: _name, ...spec }: Profile): ProfileSpec {
  return spec;
}

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

export interface Text {
  text: string;
  bold: boolean;
  wide: boolean;
  tall: boolean;
  /** Red on impact, inverse on thermal. */
  accent: boolean;
}

export interface TextLayout {
  /** At normal size, whatever the chosen width. */
  columns: number;
  lines: string[];
}

export const DEFAULT_TEXT: Text = {
  text: "",
  bold: false,
  wide: false,
  tall: false,
  accent: false,
};

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
  job: Job;
}

export interface Schedule extends ScheduleSpec {
  id: string;
}

export interface Server {
  url: string;
  token: string;
}

/** A job, as JSON or, with an image beside it, as a form. */
function jobBody(job: Job, image: File | null): RequestInit {
  const request = JSON.stringify({ job });
  if (!image) {
    return {
      body: request,
      headers: { "Content-Type": "application/json" },
    };
  }
  const form = new FormData();
  form.append("job", request);
  form.append("image", image);
  return { body: form };
}

function jsonBody(value: unknown): RequestInit {
  return {
    body: JSON.stringify(value),
    headers: { "Content-Type": "application/json" },
  };
}

export function createClient({ url, token }: Server) {
  /** The response, or the problem's `detail` thrown. */
  const send = async (method: string, path: string, init: RequestInit = {}) => {
    const response = await fetch(url + path, {
      ...init,
      method,
      headers: { ...init.headers, Authorization: `Bearer ${token}` },
    });
    if (!response.ok) throw new Error((await response.json()).detail);
    return response;
  };
  const json = <T>(method: string, path: string, init?: RequestInit) =>
    send(method, path, init).then((response) => response.json() as Promise<T>);
  const printer = (name: string) => `/v1/printers/${encodeURIComponent(name)}`;

  return {
    printers: () =>
      json<{ printers: Profile[] }>("GET", "/v1/printers").then(
        (list) => list.printers,
      ),
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
      json<Preview>("POST", `${printer(name)}/preview`, jobBody(job, image)),
    schedules: () => json<Schedule[]>("GET", "/v1/schedules"),
    createSchedule: (spec: ScheduleSpec) =>
      json<Schedule>("POST", "/v1/schedules", jsonBody(spec)),
    replaceSchedule: (id: string, spec: ScheduleSpec) =>
      json<Schedule>("PUT", `/v1/schedules/${id}`, jsonBody(spec)),
    deleteSchedule: (id: string) => send("DELETE", `/v1/schedules/${id}`),
  };
}

export type Client = ReturnType<typeof createClient>;
