import { useLayoutEffect, useRef, useState } from "react";
import type { Layout, PrinterKind } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  layout: Layout | null;
  kind: PrinterKind;
}

/** Font size of the strip's normal-size text, before scaling to fit. */
const BASE_PX = 13;
/** Horizontal padding of the strip, in normal-size characters. */
const MARGIN_CH = 2;

/**
 * Draws the card as the printer will lay it out. The strip is
 * `columns` characters wide at normal size and the task is printed at
 * double width and height. The strip is rendered at a fixed size and
 * then scaled down to fit its container, so nothing is ever clipped.
 */
export function CardPreview({ layout, kind }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const strip = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);
  const [height, setHeight] = useState(0);

  useLayoutEffect(() => {
    const outer = container.current;
    const inner = strip.current;
    if (!outer || !inner) return;

    const fit = () => {
      const available = outer.clientWidth;
      const natural = inner.offsetWidth;
      const next = natural > available ? available / natural : 1;
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
    <div ref={container} className="relative w-full" style={{ height }}>
      <div
        ref={strip}
        className="absolute top-0 left-1/2 bg-white text-black shadow-md ring-1 ring-black/10 font-mono whitespace-pre"
        style={{
          fontSize: BASE_PX,
          lineHeight: 1.3,
          width: `${columns + MARGIN_CH * 2}ch`,
          padding: `1.5em ${MARGIN_CH}ch 3em`,
          transform: `translateX(-50%) scale(${scale})`,
          transformOrigin: "top center",
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
