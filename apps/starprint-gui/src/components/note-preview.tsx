import { PrintedText } from "@/components/printed-text";
import type { Note, NoteLayout, Paper, PrinterKind } from "@/lib/api";
import { PX_PER_MM, roll } from "@/lib/paper";

interface Props {
  layout: NoteLayout | null;
  note: Note;
  kind: PrinterKind;
  paper: Paper;
}

/** A grid dot; `DOT_MM` in src-tauri/src/note.rs. */
const DOT = Math.max(1, Math.round(0.3 * PX_PER_MM));

function upTo(last: number, first = 0) {
  return Array.from({ length: last - first + 1 }, (_, index) => first + index);
}

/**
 * Draws the slip as it will print: the date, then the ruling, laid out
 * in the millimetres the printer is given.
 */
export function NotePreview({ layout, note, kind, paper }: Props) {
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
    <>
      <PrintedText columns={layout?.columns ?? 48}>
        <div>{layout?.date ?? ""}</div>
        <div>{"\n"}</div>
      </PrintedText>

      <div className="relative w-full" style={{ height }}>
        {note.rule !== "blank" &&
          note.rule !== "dots" &&
          rows.map((row) => (
            <div
              key={`rule-${row}`}
              className="absolute bg-black"
              style={{ top: down(row), height: 1, left, width: right - left }}
            />
          ))}
        {note.rule === "squares" &&
          columns.map((square) => (
            <div
              key={`side-${square}`}
              className="absolute inset-y-0 bg-black"
              style={{ left: across(square), width: 1 }}
            />
          ))}
        {note.rule === "dots" &&
          rows.map((row) =>
            columns.map((square) => (
              <div
                key={`dot-${row}-${square}`}
                className="absolute bg-black"
                style={{
                  top: down(row),
                  left: across(square),
                  width: DOT,
                  height: DOT,
                }}
              />
            )),
          )}
      </div>
    </>
  );
}
