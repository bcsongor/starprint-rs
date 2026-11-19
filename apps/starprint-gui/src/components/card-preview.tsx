import { PrintedText } from "@/components/printed-text";
import { PreviewPane } from "@/components/preview-pane";
import type { Profile, TaskCardLayout } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  /** One for each card, in print order; null until the first lands. */
  layouts: TaskCardLayout[] | null;
  error: string | null;
  printer: Profile;
}

/** Every card the form prints, one sheet each, in a pane that scrolls. */
export function CardPreview({ layouts, error, printer }: Props) {
  const first = layouts?.at(0);
  const count = layouts?.length ?? 0;
  const hint =
    first &&
    (count > 1
      ? `${count} cards · ${first.columns} columns`
      : `${first.columns} columns`);

  return (
    <PreviewPane
      hint={hint}
      printer={printer}
      error={error}
      sheets={(layouts ?? [null]).map((layout, index) => (
        <Card key={index} layout={layout} printer={printer} />
      ))}
    />
  );
}

/**
 * Draws a card as the printer will lay it out: the priority banner, the
 * reference and the due date on one line, blank without them, then the
 * task at double width and height.
 */
function Card({
  layout,
  printer,
}: {
  layout: TaskCardLayout | null;
  printer: Profile;
}) {
  const lines = layout?.lines ?? [];
  const empty = lines.every((line) => line === "");

  return (
    <PrintedText columns={layout?.columns ?? 48}>
      <div className="font-bold">
        {layout?.priority && (
          <span
            className={cn(
              printer.kind === "impact"
                ? "text-red-600"
                : "bg-black text-white",
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
      <div
        className={cn("font-bold", empty && "text-black/30")}
        style={{ fontSize: "2em", lineHeight: 1.3 }}
      >
        {empty ? "Your task" : lines.join("\n")}
      </div>
    </PrintedText>
  );
}
