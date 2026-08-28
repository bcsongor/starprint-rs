import { useLayoutEffect, useRef, useState } from "react";
import type { Layout, PrinterKind } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  layout: Layout | null;
  kind: PrinterKind;
}

/** Size the strip is laid out at before it is scaled to the print
 * region; any size does, this one keeps the scaling factor small. */
const BASE_PX = 16;

/**
 * Draws the card as the printer will lay it out: `columns` characters of
 * Font A across the print region, with the task at double width and
 * height. Font A is 12 dots wide, so a full line of it is the print
 * region exactly; the strip is laid out at [`BASE_PX`] and then scaled to
 * that width, whatever the monospace font's own metrics.
 */
export function CardPreview({ layout, kind }: Props) {
  const region = useRef<HTMLDivElement>(null);
  const strip = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  const [height, setHeight] = useState(0);

  useLayoutEffect(() => {
    const outer = region.current;
    const inner = strip.current;
    if (!outer || !inner) return;

    const fit = () => {
      // Layout widths, not painted ones: the sheet around us is itself
      // scaled, which offsetWidth ignores and a bounding rect would not.
      const next = outer.clientWidth / inner.offsetWidth;
      setScale(next);
      setHeight(inner.offsetHeight * next);
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(outer);
    observer.observe(inner);
    return () => observer.disconnect();
  }, [layout]);

  const columns = layout?.columns ?? 48;
  const hasHeader = Boolean(layout?.priority || layout?.due);
  const lines = layout?.lines.filter((line) => line !== "") ?? [];
  const empty = lines.length === 0;

  return (
    <div ref={region} className="relative w-full" style={{ height }}>
      <div
        ref={strip}
        className="absolute top-0 left-0 origin-top-left font-mono whitespace-pre"
        style={{
          fontSize: BASE_PX,
          lineHeight: 1.3,
          width: `${columns}ch`,
          transform: `scale(${scale})`,
        }}
      >
        {hasHeader && (
          <div className="font-bold">
            {layout?.priority && (
              <span
                className={cn(
                  kind === "impact" ? "text-red-600" : "bg-black text-white",
                )}
              >
                {layout.priority}
              </span>
            )}
            {layout?.due && <span className="font-normal">{layout.due}</span>}
            {"\n\n"}
          </div>
        )}
        <div
          className={cn("font-bold", empty && "text-black/30")}
          style={{ fontSize: "2em", lineHeight: 1.3 }}
        >
          {empty ? "Your task" : lines.join("\n")}
        </div>
      </div>
    </div>
  );
}
