import { invoke } from "@tauri-apps/api/core";

/** Mirrors `PrinterKind` in src-tauri/src/lib.rs. */
export type PrinterKind = "thermal" | "impact";

/** Mirrors `Printer` in src-tauri/src/lib.rs. */
export interface Printer {
  kind: PrinterKind;
  host: string;
  port: number;
  density: number;
  slow: boolean;
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

export interface PrintReport {
  bytes: number;
}

export function taskCardLayout(card: TaskCard, kind: PrinterKind) {
  return invoke<Layout>("task_card_layout", { card, kind });
}

export function printTaskCard(card: TaskCard, printer: Printer) {
  return invoke<PrintReport>("print_task_card", { card, printer });
}

export function taskCardHexdump(card: TaskCard, printer: Printer) {
  return invoke<string>("task_card_hexdump", { card, printer });
}
