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
 * Width and height are independent on these printers, so the font size
 * carries the height and a horizontal scale makes up the width.
 */
export function TextPreview({ layout, text, kind }: Props) {
  const lines = layout?.lines ?? [];
  const empty = lines.every((line) => line === "");
  const height = text.tall ? 2 : 1;
  const width = text.wide ? 2 : 1;

  return (
    <PrintedText columns={layout?.columns ?? 48}>
      <div
        // A full-width box under `scaleX` would paint past the paper.
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
