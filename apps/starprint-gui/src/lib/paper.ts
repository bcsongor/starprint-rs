import type { ProfileSpec } from "@/lib/api";

/** A roll and the print region centred on it. */
export interface Roll {
  paperMm: number;
  printMm: number;
  /** At single density. */
  dots: number;
}

/** The library crate's device profiles as geometry: 8 dots/mm on thermal,
 * 210 dots in 63 mm on the SP700. */
export function roll(printer: ProfileSpec): Roll {
  if (printer.kind === "impact") return { paperMm: 76, printMm: 63, dots: 210 };
  return printer.paper === 112
    ? { paperMm: 112, printMm: 104, dots: 832 }
    : { paperMm: 80, printMm: 72, dots: 576 };
}

/** One pixel per thermal dot, so a picture preview never scales up. */
export const PX_PER_MM = 8;
