import { invoke } from "@tauri-apps/api/core";

/** Mirrors `PrinterKind` in src-tauri/src/lib.rs. */
export type PrinterKind = "thermal" | "impact";

/** Mirrors `Speed` in src-tauri/src/lib.rs. */
export type Speed = "high" | "medium" | "slow";

/** Mirrors `Paper` in src-tauri/src/lib.rs. */
export type Paper = "80" | "112";

/** Mirrors `Printer` in src-tauri/src/lib.rs. */
export interface Printer {
  kind: PrinterKind;
  host: string;
  port: number;
  density: number;
  speed: Speed;
  paper: Paper;
  cut: boolean;
}

/** Mirrors `TaskCard` in src-tauri/src/task_card.rs. */
export interface TaskCard {
  text: string;
  priority: boolean;
  /** ISO date or free text. */
  due: string | null;
}

/** Mirrors `Layout` in src-tauri/src/task_card.rs. */
export interface Layout {
  columns: number;
  priority: string | null;
  due: string | null;
  lines: string[];
}

/** Mirrors `Text` in src-tauri/src/text.rs. */
export interface Text {
  text: string;
  bold: boolean;
  wide: boolean;
  tall: boolean;
  /** Red on impact, inverse on thermal. */
  accent: boolean;
}

/** Mirrors `Layout` in src-tauri/src/text.rs. */
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

/** Mirrors `TestPage` in src-tauri/src/test_page.rs. */
export interface TestPage {
  doubleResolution: boolean;
}

/** Mirrors `Section` in src-tauri/src/test_page.rs. */
export interface Section {
  title: string;
  check: string;
}

/** Mirrors `Dither` in src-tauri/src/picture.rs. */
export type Dither = "floyd-steinberg" | "atkinson" | "threshold" | "bayer";

/** Mirrors `Picture` in src-tauri/src/picture.rs. */
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

/** Mirrors `Job` in src-tauri/src/lib.rs. */
export type Job =
  | ({ kind: "task-card" } & TaskCard)
  | ({ kind: "text" } & Text)
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
