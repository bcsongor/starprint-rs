import { PrintedText } from "@/components/printed-text";
import { PreviewPane } from "@/components/preview-pane";
import { usePreview } from "@/hooks/use-preview";
import {
  noteLayout,
  notePreview,
  type Note,
  type Paper,
  type PrinterKind,
} from "@/lib/api";
import { PX_PER_MM, roll } from "@/lib/paper";

interface Props {
  note: Note;
  kind: PrinterKind;
  paper: Paper;
}

/** The date and the workflow's ruling bitmap, sized in millimetres. */
export function NotePreview({ note, kind, paper }: Props) {
  const { value: layout, url, error } = usePreview(
    async () => {
      const [value, png] = await Promise.all([
        noteLayout(note, kind, paper),
        notePreview(note, kind, paper),
      ]);
      return { value, png };
    },
    [note, kind, paper],
  );

  return (
    <PreviewPane
      printer={{ kind, paper }}
      hint={`${note.rows} rows, ${note.rows * note.pitch} mm`}
    >
      {error ? (
        <p className="py-4 text-center text-sm text-destructive">{error}</p>
      ) : layout && url ? (
        <>
          <PrintedText columns={layout.columns}>
            <div>{layout.date}</div>
            <div>{"\n"}</div>
          </PrintedText>
          <img
            src={url}
            alt="Note ruling preview"
            className="block"
            width={roll(kind, paper).printMm * PX_PER_MM}
            height={note.rows * note.pitch * PX_PER_MM}
          />
        </>
      ) : null}
    </PreviewPane>
  );
}
