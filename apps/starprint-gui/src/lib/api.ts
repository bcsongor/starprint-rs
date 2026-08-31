import { invoke } from "@tauri-apps/api/core";

/** Mirrors `PrinterKind` in starprint-workflows/src/lib.rs. */
export type PrinterKind = "thermal" | "impact";

/** Mirrors `Speed` in starprint-workflows/src/lib.rs. */
export type Speed = "high" | "medium" | "slow";

/** Mirrors `Paper` in starprint-workflows/src/lib.rs. */
export type Paper = "80" | "112";

/** Mirrors `Printer` in starprint-workflows/src/lib.rs. */
export interface Printer {
  kind: PrinterKind;
  host: string;
  port: number;
  density: number;
  speed: Speed;
  paper: Paper;
  cut: boolean;
}

/** Mirrors `TaskCard` in starprint-workflows/src/task_card.rs. */
export interface TaskCard {
  text: string;
  priority: boolean;
  /** ISO date or free text. */
  due: string | null;
}

/** Mirrors `Layout` in starprint-workflows/src/task_card.rs. */
export interface Layout {
  columns: number;
  priority: string | null;
  due: string | null;
  lines: string[];
}

/** Mirrors `Text` in starprint-workflows/src/text.rs. */
export interface Text {
  text: string;
  bold: boolean;
  wide: boolean;
  tall: boolean;
  /** Red on impact, inverse on thermal. */
  accent: boolean;
}

/** Mirrors `Layout` in starprint-workflows/src/text.rs. */
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

/** Mirrors `Rule` in starprint-workflows/src/note.rs. */
export type Rule = "blank" | "dots" | "lines" | "squares";

/** Mirrors `Note` in starprint-workflows/src/note.rs. */
export interface Note {
  rule: Rule;
  /** Rows to write in. */
  rows: number;
  /** Millimetres between the rules, across and down. */
  pitch: number;
}

/** Mirrors `Layout` in starprint-workflows/src/note.rs. */
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

/** Mirrors `Align` in starprint-workflows/src/qr.rs. */
export type Align = "left" | "center" | "right";

/** Mirrors `Ecc` in starprint-workflows/src/qr.rs. */
export type Ecc = "l" | "m" | "q" | "h";

/** How much of a symbol each level can lose and still scan. */
export const RECOVERY: Record<Ecc, string> = {
  l: "7%",
  m: "15%",
  q: "25%",
  h: "30%",
};

/** Mirrors `Qr` in starprint-workflows/src/qr.rs. */
export interface Qr {
  data: string;
  /** Printed above the symbol. */
  caption: string | null;
  errorCorrection: Ecc;
  /** The symbol's width in millimetres, quiet zone excluded. */
  size: number;
  /**
   * How far a module's corners are taken off, as a percentage of half
   * the module: 100 draws a lone module as a circle, 0 leaves it
   * square. Rounded down to whole dots, so a coarse head prints square
   * whatever this asks for.
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
  radius: MAX_QR_RADIUS,
  align: "center",
};

/** Mirrors `Layout` in starprint-workflows/src/qr.rs. */
export interface QrLayout {
  columns: number;
  /** Wrapped to the paper; empty when there is no caption. */
  caption: string[];
  align: Align;
  /** Modules across the block, quiet zone included. */
  modules: number;
  /** Row-major, `true` is dark. `modules * modules` long. */
  matrix: boolean[];
  /**
   * The corner radius a module is drawn with, as a fraction of the
   * module across and down. Zero where the head's dots are too coarse
   * to round, which is the SP700 at the sizes it is usually asked for.
   */
  radiusX: number;
  radiusY: number;
  /** What the block measures on paper, quiet zone included. */
  widthMm: number;
  heightMm: number;
}

/** The quiet zone each side; `QUIET_MODULES` in the same file. */
export const QUIET_MODULES = 4;

/** Mirrors `TestPage` in starprint-workflows/src/test_page.rs. */
export interface TestPage {
  doubleResolution: boolean;
}

/** Mirrors `Section` in starprint-workflows/src/test_page.rs. */
export interface Section {
  title: string;
  check: string;
}

/** Mirrors `Dither` in starprint-workflows/src/picture.rs. */
export type Dither = "floyd-steinberg" | "atkinson" | "threshold" | "bayer";

/**
 * Mirrors `Picture` in starprint-workflows/src/picture.rs, with the
 * `path` that `PicturePath` in src-tauri/src/source.rs adds. The shared
 * crate takes image data, so choosing a file is the app's own business.
 */
export interface Picture {
  /** Empty until one is chosen. */
  path: string;
  /** Impact: double density. Thermal: double-resolution mode. */
  double: boolean;
  dither: Dither;
  /** Ignored by Bayer. */
  threshold: number;
  brightness: number;
  contrast: number;
}

export const DEFAULT_PICTURE: Picture = {
  path: "",
  double: false,
  dither: "floyd-steinberg",
  threshold: 128,
  brightness: 1,
  contrast: 1,
};

/** Mirrors `JobRequest` in src-tauri/src/source.rs. */
export type Job =
  | ({ kind: "task-card" } & TaskCard)
  | ({ kind: "text" } & Text)
  | ({ kind: "note" } & Note)
  | ({ kind: "qr" } & Qr)
  | ({ kind: "test-page" } & TestPage)
  | ({ kind: "picture" } & Picture);

export interface PrintReport {
  bytes: number;
}

export interface HexDump {
  bytes: number;
  /** 16 bytes per row: offset, hex, ASCII. */
  dump: string;
}

export function taskCardLayout(
  card: TaskCard,
  kind: PrinterKind,
  paper: Paper,
) {
  return invoke<Layout>("task_card_layout", { card, kind, paper });
}

export function textLayout(text: Text, kind: PrinterKind, paper: Paper) {
  return invoke<TextLayout>("text_layout", { text, kind, paper });
}

export function noteLayout(note: Note, kind: PrinterKind, paper: Paper) {
  return invoke<NoteLayout>("note_layout", { note, kind, paper });
}

/** Rejects data too long to encode. */
export function qrLayout(code: Qr, kind: PrinterKind, paper: Paper) {
  return invoke<QrLayout>("qr_layout", { code, kind, paper });
}

export function testPageSections(page: TestPage, kind: PrinterKind) {
  return invoke<Section[]>("test_page_sections", { page, kind });
}

export function printJob(job: Job, printer: Printer) {
  return invoke<PrintReport>("print_job", { job, printer });
}

export function jobHexdump(job: Job, printer: Printer) {
  return invoke<HexDump>("job_hexdump", { job, printer });
}

/** PNG bytes. */
export function picturePreview(
  picture: Picture,
  kind: PrinterKind,
  paper: Paper,
) {
  return invoke<ArrayBuffer>("picture_preview", { picture, kind, paper });
}

export function probePrinter(host: string, port: number) {
  return invoke<boolean>("probe_printer", { host, port });
}
