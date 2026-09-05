import { PrintedText } from "@/components/printed-text";
import { PreviewPane } from "@/components/preview-pane";
import { useAsync } from "@/hooks/use-async";
import { noteLayout, type Note, type Paper, type PrinterKind } from "@/lib/api";
import { PX_PER_MM, roll } from "@/lib/paper";

interface Props {
  note: Note;
  kind: PrinterKind;
  paper: Paper;
}

/** A grid dot; `DOT_MM` in starprint-workflows/src/note.rs. */
const DOT = Math.max(1, Math.round(0.3 * PX_PER_MM));

function upTo(last: number, first = 0) {
  return Array.from({ length: last - first + 1 }, (_, index) => first + index);
}

/**
 * Draws the slip as it will print: the date, then the ruling, laid out
 * in the millimetres the printer is given.
 */
export function NotePreview({ note, kind, paper }: Props) {
  const layout = useAsync(
    () => noteLayout(note, kind, paper),
    [note, kind, paper],
  );
  const width = roll(kind, paper).printMm * PX_PER_MM;
  const pitch = note.pitch * PX_PER_MM;
  const height = note.rows * pitch;
  // Whole squares, centred, so the ruling is not lopsided.
  const squares = Math.floor(width / pitch);
  const margin = (width - squares * pitch) / 2;

  // A rule is pulled back to fit rather than clipped, as it is in print.
  const thickness = note.rule === "dots" ? DOT : 1;
  const down = (row: number) => Math.min(row * pitch, height - thickness);
  const across = (square: number) =>
    Math.min(margin + square * pitch, width - thickness);

  // Ruled paper is written on top of the rule, so it has none at the
  // top; a grid is closed on all sides.
  const rows = upTo(note.rows, note.rule === "lines" ? 1 : 0);
  const columns = note.rule === "blank" ? [] : upTo(squares);
  // Squared paper is a closed block, so its rules stop at the outermost
  // sides rather than running on to the edge of the paper.
  const left = note.rule === "squares" ? across(0) : 0;
  const right = note.rule === "squares" ? across(squares) + 1 : width;

  return (
    <PreviewPane
      printer={{ kind, paper }}
      hint={`${note.rows} rows, ${note.rows * note.pitch} mm`}
    >
      <PrintedText columns={layout?.columns ?? 48}>
        <div>{layout?.date ?? ""}</div>
        <div>{"\n"}</div>
      </PrintedText>

      <svg
        className="block overflow-visible"
        width={width}
        height={height}
        shapeRendering={note.rule === "dots" ? "auto" : "crispEdges"}
        aria-hidden
      >
        {note.rule !== "blank" &&
          note.rule !== "dots" &&
          rows.map((row) => (
            <line
              key={`rule-${row}`}
              x1={left}
              x2={right}
              y1={down(row) + 0.5}
              y2={down(row) + 0.5}
              stroke="black"
              vectorEffect="non-scaling-stroke"
            />
          ))}
        {note.rule === "squares" &&
          columns.map((square) => (
            <line
              key={`side-${square}`}
              x1={across(square) + 0.5}
              x2={across(square) + 0.5}
              y1={0}
              y2={height}
              stroke="black"
              vectorEffect="non-scaling-stroke"
            />
          ))}
        {note.rule === "dots" &&
          rows.map((row) =>
            columns.map((square) => (
              <rect
                key={`dot-${row}-${square}`}
                x={across(square)}
                y={down(row)}
                width={DOT}
                height={DOT}
              />
            )),
          )}
      </svg>
    </PreviewPane>
  );
}
