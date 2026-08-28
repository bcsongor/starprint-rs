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
  /** Thermal only: the roll loaded in the printer. */
  paper: Paper;
  /** Cut the paper after the job. */
  cut: boolean;
}

/** Mirrors `TaskCard` in src-tauri/src/task_card.rs. */
export interface TaskCard {
  text: string;
  priority: boolean;
  /** ISO date (`2026-08-28`) or free text; null for no due date. */
  due: string | null;
}

/** Mirrors `Layout` in src-tauri/src/task_card.rs. */
export interface Layout {
  columns: number;
  priority: string | null;
  due: string | null;
  lines: string[];
}

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
  /** Path of the image file on disk; empty until one is chosen. */
  path: string;
  /** Impact: double density. Thermal: double-resolution mode. */
  double: boolean;
  dither: Dither;
  /** 1..255; ignored by Bayer. */
  threshold: number;
  /** 1.0 leaves the picture as is. */
  brightness: number;
  /** 1.0 leaves the picture as is. */
  contrast: number;
}

/** Mirrors `Job` in src-tauri/src/lib.rs. */
export type Job =
  | ({ kind: "task-card" } & TaskCard)
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

export function taskCardLayout(card: TaskCard, kind: PrinterKind, paper: Paper) {
  return invoke<Layout>("task_card_layout", { card, kind, paper });
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

/** The dithered picture as a PNG, through the dot model on thermal. */
export function picturePreview(
  picture: Picture,
  kind: PrinterKind,
  paper: Paper,
) {
  return invoke<ArrayBuffer>("picture_preview", { picture, kind, paper });
}

/** True when the printer answers on its port. */
export function probePrinter(host: string, port: number) {
  return invoke<boolean>("probe_printer", { host, port });
}
