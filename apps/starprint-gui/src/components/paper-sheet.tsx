import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { Paper, PrinterKind } from "@/lib/api";
import { PX_PER_MM, roll } from "@/lib/paper";

interface Props {
  kind: PrinterKind;
  paper: Paper;
  /** Drawn across the print region, which is as many pixels wide as the
   * head has dots. */
  children: ReactNode;
}

/** Paper above the first line and below the last, as the feed leaves it. */
const TOP_MM = 5;
const BOTTOM_MM = 8;

/**
 * A normal-size character cell in sheet pixels. Font A is 12 dots wide
 * on a thermal head, which is 12 px here; the SP700 fits 42 columns into
 * 210 dots across 63 mm, which comes to the same 12 px.
 */
const CELL_PX = 12;

/** Characters the probe measures, and the size it measures them at. */
const PROBE = "0".repeat(10);
const PROBE_PX = 100;

/**
 * The paper every preview is drawn on: the roll at its true width with
 * the print region centred on it, scaled down as far as the pane needs
 * but never up, so a picture keeps one screen pixel per dot.
 *
 * The sheet also sets the size of printed text, so anything drawn on it
 * comes out at the printer's own normal size unless it says otherwise.
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
    // The monospace font's advance as a fraction of its size, from which
    // the size that puts one character in one cell follows.
    const advance = el.offsetWidth / PROBE.length / PROBE_PX;
    setPrintPx(CELL_PX / advance);
  }, []);

  return (
    <div className="relative min-h-0 flex-1">
      {/* The pane is measured on its own so that the sheet's height,
          which the scale depends on, cannot feed back into it. */}
      <div ref={pane} className="absolute inset-0" />
      <div
        ref={sheet}
        className="absolute top-0 left-1/2 origin-top bg-white font-mono text-black shadow-md ring-1 ring-black/10"
        style={{
          width,
          fontSize: printPx,
          lineHeight: 1.3,
          padding: `${TOP_MM * PX_PER_MM}px ${edge}px ${BOTTOM_MM * PX_PER_MM}px`,
          transform: `translateX(-50%) scale(${scale})`,
        }}
      >
        <span
          ref={probe}
          aria-hidden
          className="invisible absolute whitespace-pre"
          style={{ fontSize: PROBE_PX }}
        >
          {PROBE}
        </span>
        {children}
      </div>
    </div>
  );
}
