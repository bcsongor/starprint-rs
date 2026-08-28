import { cn } from "@/lib/utils";
import type { Layout, PrinterKind } from "@/lib/api";

interface Props {
  layout: Layout | null;
  kind: PrinterKind;
}

/**
 * Draws the card as the printer will lay it out: the strip is `columns`
 * characters wide, the task is double width and height. It is a
 * monospace mock-up, not a raster of the printer's font.
 */
export function CardPreview({ layout, kind }: Props) {
  const columns = layout?.columns ?? 48;
  const hasHeader = Boolean(layout?.priority || layout?.due);

  return (
    <div className="flex justify-center">
      <div
        className="bg-white text-black shadow-md ring-1 ring-black/10 font-mono text-[11px] leading-[1.35] px-3 pt-6 pb-10 whitespace-pre overflow-hidden"
        style={{ width: `calc(${columns}ch + 1.5rem)` }}
      >
        {hasHeader && (
          <div className="font-bold">
            {layout?.priority && (
              <span
                className={cn(
                  kind === "impact"
                    ? "text-red-600"
                    : "bg-black text-white",
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
          className="font-bold origin-top-left"
          style={{ transform: "scale(2)", width: "50%" }}
        >
          {layout?.lines.length ? layout.lines.join("\n") : " "}
        </div>
        {/* The scaled block does not take up layout space; reserve it. */}
        <div
          aria-hidden
          style={{ height: `${Math.max(1, layout?.lines.length ?? 1) * 1.35 * 2}em` }}
        />
      </div>
    </div>
  );
}
