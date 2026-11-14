import { PrintedText } from "@/components/printed-text";
import { PreviewPane } from "@/components/preview-pane";
import type { Note, Profile } from "@/lib/api";
import type { Previews } from "@/lib/job";
import { PX_PER_MM, roll } from "@/lib/paper";

interface Props {
  note: Note;
  preview: Previews["note"] | null;
  error: string | null;
  printer: Profile;
}

/** The date and the workflow's ruling bitmap, sized in millimetres. */
export function NotePreview({ note, preview, error, printer }: Props) {
  return (
    <PreviewPane
      printer={printer}
      hint={`${note.rows} rows, ${note.rows * note.pitch} mm`}
      error={error}
    >
      {preview && (
        <>
          <PrintedText columns={preview.columns}>
            <div>{preview.date}</div>
            <div>{"\n"}</div>
          </PrintedText>
          <img
            src={preview.image}
            alt="Note ruling preview"
            className="block"
            width={roll(printer).printMm * PX_PER_MM}
            height={note.rows * note.pitch * PX_PER_MM}
          />
        </>
      )}
    </PreviewPane>
  );
}
