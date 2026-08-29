import {
  AlignJustifyIcon,
  Grid3x3Icon,
  GripIcon,
  SquareIcon,
  type LucideIcon,
} from "lucide-react";
import { SliderField } from "@/components/slider-field";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { NOTEBOOK_RULING, type Note, type Rule } from "@/lib/api";

const RULES: { value: Rule; label: string; Icon: LucideIcon }[] = [
  { value: "blank", label: "Blank", Icon: SquareIcon },
  { value: "dots", label: "Dots", Icon: GripIcon },
  { value: "lines", label: "Lines", Icon: AlignJustifyIcon },
  { value: "squares", label: "Squares", Icon: Grid3x3Icon },
];

/** `MAX_ROWS` and the pitch range in src-tauri/src/note.rs. */
const MAX_ROWS = 20;
const MIN_PITCH_MM = 4;
const MAX_PITCH_MM = 12;

interface Props {
  note: Note;
  onChange: (note: Note) => void;
}

export function NoteForm({ note, onChange }: Props) {
  const set = <K extends keyof Note>(key: K, value: Note[K]) =>
    onChange({ ...note, [key]: value });

  return (
    <FieldGroup className="gap-4">
      <Field>
        {/* Same row height as the Task label, so the tabs do not shift. */}
        <div className="flex h-5 items-center">
          <FieldLabel render={<span />}>Ruling</FieldLabel>
        </div>
        {/* Buttons, not toggles: a toggle's tint is too quiet to read. */}
        <div className="flex items-center gap-2">
          {RULES.map(({ value, label, Icon }) => (
            <Button
              key={value}
              variant={note.rule === value ? "default" : "secondary"}
              className="px-2.5"
              aria-pressed={note.rule === value}
              // Each ruling comes at its standard pitch, and at the rows
              // that keep the slip the same length; the sliders are
              // there to argue with it.
              onClick={() => onChange({ rule: value, ...NOTEBOOK_RULING[value] })}
            >
              <Icon />
              {label}
            </Button>
          ))}
        </div>
      </Field>

      <SliderField
        id="rows"
        label="Rows"
        value={note.rows}
        display={String(note.rows)}
        min={1}
        max={MAX_ROWS}
        onChange={(value) => set("rows", value)}
      />
      <SliderField
        id="pitch"
        label="Pitch"
        value={note.pitch}
        display={`${note.pitch} mm`}
        min={MIN_PITCH_MM}
        max={MAX_PITCH_MM}
        onChange={(value) => set("pitch", value)}
      />
    </FieldGroup>
  );
}
