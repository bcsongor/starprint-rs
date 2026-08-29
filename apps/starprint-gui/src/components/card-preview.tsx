import { PrintedText } from "@/components/printed-text";
import type { Layout, PrinterKind } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  layout: Layout | null;
  kind: PrinterKind;
}

/**
 * Draws the card as the printer will lay it out: the priority banner and
 * the due date on one line, then the task at double width and height.
 */
export function CardPreview({ layout, kind }: Props) {
  const hasHeader = Boolean(layout?.priority || layout?.due);
  const lines = layout?.lines.filter((line) => line !== "") ?? [];
  const empty = lines.length === 0;

  return (
    <PrintedText columns={layout?.columns ?? 48}>
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
    </PrintedText>
  );
}
