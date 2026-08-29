import { useLayoutEffect, useRef, useState, type ReactNode } from "react";

/** Size the strip is laid out at before it is scaled to the print
 * region; any size does, this one keeps the scaling factor small. */
const BASE_PX = 16;

interface Props {
  /** Characters per line at normal size, which fill the print region. */
  columns: number;
  children: ReactNode;
}

/**
 * Text at the size it prints. A character of Font A is 12 dots wide, so
 * a full line of it is the print region exactly: the strip is laid out
 * at [`BASE_PX`] and then scaled to that width, whatever the monospace
 * font's own metrics. Sizes inside it are relative, so `2em` is what the
 * printer calls double.
 */
export function PrintedText({ columns, children }: Props) {
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
  }, [columns, children]);

  return (
    <div ref={region} className="relative w-full" style={{ height }}>
      <div
        ref={strip}
        className="absolute top-0 left-0 origin-top-left whitespace-pre"
        style={{
          fontSize: BASE_PX,
          lineHeight: 1.3,
          width: `${columns}ch`,
          transform: `scale(${scale})`,
        }}
      >
        {children}
      </div>
    </div>
  );
}
