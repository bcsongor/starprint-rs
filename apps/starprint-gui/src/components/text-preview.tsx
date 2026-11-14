import { Fragment } from "react";
import { PrintedText } from "@/components/printed-text";
import { PreviewPane } from "@/components/preview-pane";
import type { PrinterKind, Profile, Run, TextLayout } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  /** Null until the first layout lands. */
  layout: TextLayout | null;
  error: string | null;
  printer: Profile;
}

export function TextPreview({ layout, error, printer }: Props) {
  const lines = layout?.lines ?? [];
  const empty = lines.every((line) => line.every((run) => run.text === ""));

  return (
    <PreviewPane
      printer={printer}
      hint={layout && `${layout.columns} columns`}
      error={error}
    >
      <PrintedText columns={layout?.columns ?? 48}>
        {empty ? (
          <span className="text-black/30">Your text</span>
        ) : (
          lines.map((line, index) => (
            <Fragment key={index}>
              {index > 0 && "\n"}
              {line.map((run, index) => (
                <PrintedRun key={index} run={run} kind={printer.kind} />
              ))}
            </Fragment>
          ))
        )}
      </PrintedText>
    </PreviewPane>
  );
}

/**
 * Width and height are independent on these printers, so the font size
 * carries the height and a horizontal scale makes up the width. The
 * scale moves nothing, so the outer box takes the columns the run does.
 */
function PrintedRun({ run, kind }: { run: Run; kind: PrinterKind }) {
  const height = run.tall ? 2 : 1;
  const width = run.wide ? 2 : 1;
  const columns = [...run.text].length * width;
  return (
    <span
      className={cn(
        "inline-block",
        run.bold && "font-bold",
        // Inverse fills the character cells that print.
        run.accent &&
          (kind === "impact" ? "text-red-600" : "bg-black text-white"),
      )}
      style={{ width: `${columns}ch` }}
    >
      <span
        className="inline-block origin-left"
        style={{
          fontSize: `${height}em`,
          transform: `scaleX(${width / height})`,
        }}
      >
        {/* A blank line's empty run would otherwise take no height. */}
        {run.text || "​"}
      </span>
    </span>
  );
}
