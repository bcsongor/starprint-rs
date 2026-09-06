import { useLayoutEffect, useRef, type ReactNode } from "react";

/** Any size does; this one keeps the scale factor small. */
const BASE_PX = 16;

interface Props {
  /** At normal size; a full line is the print region exactly. */
  columns: number;
  children: ReactNode;
}

/**
 * Lays the strip out at `columns` characters and scales it to the print
 * region, whatever the font's metrics. Sizes inside are relative, so
 * `2em` is the printer's double.
 */
export function PrintedText({ columns, children }: Props) {
  const region = useRef<HTMLDivElement>(null);
  const strip = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const outer = region.current;
    const inner = strip.current;
    if (!outer || !inner) return;

    const fit = () => {
      // offsetWidth ignores the sheet's own scale; a bounding rect would not.
      const next = outer.clientWidth / inner.offsetWidth;
      inner.style.transform = `scale(${next})`;
      // The parent sheet measures this height in its own layout effect.
      outer.style.height = `${inner.offsetHeight * next}px`;
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(outer);
    observer.observe(inner);
    return () => observer.disconnect();
  }, [columns, children]);

  return (
    <div ref={region} className="relative w-full">
      <div
        ref={strip}
        className="absolute top-0 left-0 origin-top-left whitespace-pre"
        style={{
          fontSize: BASE_PX,
          lineHeight: 1.3,
          width: `${columns}ch`,
        }}
      >
        {children}
      </div>
    </div>
  );
}
