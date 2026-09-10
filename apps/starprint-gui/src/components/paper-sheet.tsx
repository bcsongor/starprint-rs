import { useLayoutEffect, useRef, type ReactNode } from "react";
import type { Profile } from "@/lib/api";
import { PX_PER_MM, roll } from "@/lib/paper";

interface Props {
  printer: Profile;
  /** Drawn across the print region, one pixel per dot. */
  children: ReactNode;
}

/** Paper the feed leaves above the first line and below the last. */
const TOP_MM = 5;
const BOTTOM_MM = 8;

/**
 * The roll at true width with the print region centred, scaled down to
 * fit the pane but never up.
 */
export function PaperSheet({ printer, children }: Props) {
  const pane = useRef<HTMLDivElement>(null);
  const sheet = useRef<HTMLDivElement>(null);

  const { paperMm, printMm } = roll(printer);
  const width = paperMm * PX_PER_MM;
  const edge = ((paperMm - printMm) / 2) * PX_PER_MM;

  useLayoutEffect(() => {
    const outer = pane.current;
    const inner = sheet.current;
    if (!outer || !inner) return;

    const fit = () => {
      const scale = Math.min(
        1,
        outer.clientWidth / inner.offsetWidth,
        outer.clientHeight / inner.offsetHeight,
      );
      // Apply before paint, including resize callbacks outside React commits.
      inner.style.transform = `translateX(-50%) scale(${scale})`;
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(outer);
    observer.observe(inner);
    return () => observer.disconnect();
  }, [width, children]);

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
          lineHeight: 1.3,
          padding: `${TOP_MM * PX_PER_MM}px ${edge}px ${BOTTOM_MM * PX_PER_MM}px`,
        }}
      >
        {children}
      </div>
    </div>
  );
}
