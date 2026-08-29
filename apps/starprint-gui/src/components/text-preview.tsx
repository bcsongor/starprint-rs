import { Fragment } from "react";
import { PrintedText } from "@/components/printed-text";
import type { PrinterKind, Text, TextLayout } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  layout: TextLayout | null;
  text: Text;
  kind: PrinterKind;
}

/**
 * Draws the text as it will print: wrapped where the printer wraps it,
 * in the styles the head is being asked for. Double width and height are
 * separate on these printers, so the size comes from the font for the
 * height and a horizontal scale makes up the difference — which is what
 * the head does to the glyph too.
 */
export function TextPreview({ layout, text, kind }: Props) {
  const lines = layout?.lines ?? [];
  const empty = lines.every((line) => line === "");
  const height = text.tall ? 2 : 1;
  const width = text.wide ? 2 : 1;

  return (
    <PrintedText columns={layout?.columns ?? 48}>
      <div
        // Only as wide as the longest line: a full-width box stretched
        // by `scaleX` would paint past the paper and scroll the window.
        className={cn(
          "w-fit",
          text.bold && "font-bold",
          empty && "text-black/30",
        )}
        style={{
          fontSize: `${height}em`,
          lineHeight: 1.3,
          transform: `scaleX(${width / height})`,
          transformOrigin: "left top",
        }}
      >
        {empty
          ? "Your text"
          : lines.map((line, index) => (
              // Inverse fills the character cells that print, so each
              // line carries its own background rather than the block.
              <Fragment key={`${index}-${line}`}>
                {index > 0 && "\n"}
                <span
                  className={cn(
                    text.accent &&
                      (kind === "impact"
                        ? "text-red-600"
                        : "bg-black text-white"),
                  )}
                >
                  {line}
                </span>
              </Fragment>
            ))}
      </div>
    </PrintedText>
  );
}
