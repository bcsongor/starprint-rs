import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { Paper, PrinterKind } from "@/lib/api";
import { PX_PER_MM, roll } from "@/lib/paper";

interface Props {
  kind: PrinterKind;
  paper: Paper;
  /** Drawn across the print region, one pixel per dot. */
  children: ReactNode;
}

/** Paper the feed leaves above the first line and below the last. */
const TOP_MM = 5;
const BOTTOM_MM = 8;

/** A character cell: Font A is 12 dots, and 210 dots / 42 columns on the
 * SP700 comes to the same. */
const CELL_PX = 12;

const PROBE = "0".repeat(10);
const PROBE_PX = 100;

/**
 * The roll at true width with the print region centred, scaled down to
 * fit the pane but never up. Sets the font size so text on it prints at
 * the printer's normal size.
 */
export function PaperSheet({ kind, paper, children }: Props) {
  const pane = useRef<HTMLDivElement>(null);
  const sheet = useRef<HTMLDivElement>(null);
  const probe = useRef<HTMLSpanElement>(null);
  const [scale, setScale] = useState(1);
  const [printPx, setPrintPx] = useState(CELL_PX * 2);

  const { paperMm, printMm } = roll(kind, paper);
  const width = paperMm * PX_PER_MM;
  const edge = ((paperMm - printMm) / 2) * PX_PER_MM;

  useLayoutEffect(() => {
    const outer = pane.current;
    const inner = sheet.current;
    if (!outer || !inner) return;

    const fit = () =>
      setScale(
        Math.min(
          1,
          outer.clientWidth / inner.offsetWidth,
          outer.clientHeight / inner.offsetHeight,
        ),
      );
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(outer);
    observer.observe(inner);
    return () => observer.disconnect();
  }, []);

  useLayoutEffect(() => {
    const el = probe.current;
    if (!el) return;
    // The font's advance as a fraction of its size.
    const advance = el.offsetWidth / PROBE.length / PROBE_PX;
    setPrintPx(CELL_PX / advance);
  }, []);

  return (
    <div className="relative min-h-0 flex-1">
      {/* Measured apart from the sheet so the scale cannot feed back. */}
      <div ref={pane} className="absolute inset-0" />
      {/* Composited from the start. WebKit otherwise re-rasterises the
          scaled text when a popup's layer appears, and it visibly snaps. */}
      <div
        ref={sheet}
        className="absolute top-0 left-1/2 origin-top bg-white font-mono text-black shadow-md ring-1 ring-black/10 will-change-transform"
        style={{
          width,
          fontSize: printPx,
          lineHeight: 1.3,
          padding: `${TOP_MM * PX_PER_MM}px ${edge}px ${BOTTOM_MM * PX_PER_MM}px`,
          // Lay out at the display scale so thin rules can snap to pixels.
          zoom: scale,
          transform: "translateX(-50%)",
        }}
      >
        {/* Clipped to nothing: it is wider than the SP700's sheet, and
            even hidden it would scroll the window. */}
        <div aria-hidden className="absolute size-0 overflow-hidden">
          <span
            ref={probe}
            className="whitespace-pre"
            style={{ fontSize: PROBE_PX }}
          >
            {PROBE}
          </span>
        </div>
        {children}
      </div>
    </div>
  );
}
