import type { Paper, PrinterKind } from "@/lib/api";

/** A roll and the print region centred on it, in millimetres. */
export interface Roll {
  /** Width of the paper itself. */
  paperMm: number;
  /** Width the head can reach, the rest being the unprintable edges. */
  printMm: number;
  /** Dots across the print region at single density. */
  dots: number;
}

/**
 * The paper a printer is loaded with. The figures are the device profiles
 * of the library crate read as geometry: thermal heads are 8 dots/mm, so
 * 576 dots is 72 mm and 832 dots is 104 mm, and the SP700 fits 210 dots
 * into 63 mm. Roll widths are the nominal sizes those printers take.
 */
export function roll(kind: PrinterKind, paper: Paper): Roll {
  // The SP700's carriage is fixed, so its roll does not come into it.
  if (kind === "impact") return { paperMm: 76, printMm: 63, dots: 210 };
  return paper === "112"
    ? { paperMm: 112, printMm: 104, dots: 832 }
    : { paperMm: 80, printMm: 72, dots: 576 };
}

/**
 * Rendered pixels per millimetre of paper before the sheet is scaled to
 * fit its pane. One pixel per thermal dot, so a picture preview is never
 * resampled upwards.
 */
export const PX_PER_MM = 8;
