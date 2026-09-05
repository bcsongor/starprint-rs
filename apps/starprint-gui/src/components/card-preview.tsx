import { PrintedText } from "@/components/printed-text";
import { PreviewPane } from "@/components/preview-pane";
import { useAsync } from "@/hooks/use-async";
import {
  taskCardLayout,
  type Paper,
  type PrinterKind,
  type TaskCard,
} from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  card: TaskCard;
  kind: PrinterKind;
  paper: Paper;
}

/**
 * Draws the card as the printer will lay it out: the priority banner,
 * the reference and the due date on one line, then the task at double
 * width and height.
 */
export function CardPreview({ card, kind, paper }: Props) {
  const layout = useAsync(
    () => taskCardLayout(card, kind, paper),
    [card, kind, paper],
  );
  const hasHeader = Boolean(
    layout?.priority || layout?.reference || layout?.due,
  );
  const lines = layout?.lines.filter((line) => line !== "") ?? [];
  const empty = lines.length === 0;

  return (
    <PreviewPane
      printer={{ kind, paper }}
      hint={layout && `${layout.columns} columns`}
    >
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
            {layout?.reference && (
              <span className="font-normal">{layout.reference}</span>
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
    </PreviewPane>
  );
}
