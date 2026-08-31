import {
  AlignCenterIcon,
  AlignLeftIcon,
  AlignRightIcon,
  CornerDownLeftIcon,
  type LucideIcon,
} from "lucide-react";
import { SliderField } from "@/components/slider-field";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Textarea } from "@/components/ui/textarea";
import {
  MAX_QR_MM,
  MIN_QR_MM,
  RECOVERY,
  type Align,
  type Ecc,
  type Qr,
} from "@/lib/api";

const ALIGNMENTS: { value: Align; label: string; Icon: LucideIcon }[] = [
  { value: "left", label: "Left", Icon: AlignLeftIcon },
  { value: "center", label: "Centre", Icon: AlignCenterIcon },
  { value: "right", label: "Right", Icon: AlignRightIcon },
];

const LEVELS: Ecc[] = ["l", "m", "q", "h"];

interface Props {
  code: Qr;
  onChange: (code: Qr) => void;
  /** Called on Ctrl/Cmd+Enter in the data field. */
  onSubmit: () => void;
}

export function QrForm({ code, onChange, onSubmit }: Props) {
  const set = <K extends keyof Qr>(key: K, value: Qr[K]) =>
    onChange({ ...code, [key]: value });

  return (
    <FieldGroup className="gap-4">
      <Field>
        <div className="flex h-5 items-center justify-between">
          <FieldLabel htmlFor="data">Data</FieldLabel>
          <span className="flex items-center gap-1 text-xs text-muted-foreground">
            <KbdGroup>
              <Kbd>Ctrl</Kbd>
              <Kbd>
                <CornerDownLeftIcon />
              </Kbd>
            </KbdGroup>
            prints
          </span>
        </div>
        <Textarea
          id="data"
          // A payload is usually one line; the box grows if it is not.
          className="max-h-28 min-h-16 border-border bg-background text-sm"
          value={code.data}
          placeholder="A link, a phone number, WIFI:T:WPA;S:…;P:…;;"
          autoFocus
          onChange={(e) => set("data", e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              onSubmit();
            }
          }}
        />
      </Field>

      <Field>
        <FieldLabel htmlFor="caption">Caption</FieldLabel>
        <Input
          id="caption"
          className="border-border bg-background text-sm"
          value={code.caption ?? ""}
          placeholder="What the code is for"
          // Blank is no caption at all, not an empty line above the symbol.
          onChange={(e) => set("caption", e.target.value || null)}
        />
      </Field>

      {/* One row: the slider takes what the two button groups leave. */}
      <div className="flex items-end gap-3">
        <SliderField
          id="size"
          label="Size"
          value={code.size}
          display={`${code.size} mm`}
          min={MIN_QR_MM}
          max={MAX_QR_MM}
          // Pads the 4 px track out to the height of a button, so all
          // three labels in the row sit on one line.
          className="min-w-0 flex-1 [&>[data-slot=slider]]:my-3.5"
          onChange={(value) => set("size", value)}
        />

        <Field className="w-auto shrink-0">
          <FieldLabel render={<span />}>Align</FieldLabel>
          {/* Buttons, not toggles: a toggle's tint is too quiet to read. */}
          <div className="flex items-center gap-1.5">
            {ALIGNMENTS.map(({ value, label, Icon }) => (
              <Button
                key={value}
                size="icon"
                variant={code.align === value ? "default" : "secondary"}
                aria-label={label}
                aria-pressed={code.align === value}
                title={label}
                onClick={() => set("align", value)}
              >
                <Icon />
              </Button>
            ))}
          </div>
        </Field>

        <Field className="w-auto shrink-0">
          <div className="flex items-center justify-between gap-2">
            <FieldLabel render={<span />}>Recovery</FieldLabel>
            <span className="font-mono text-xs tabular-nums text-muted-foreground">
              {RECOVERY[code.errorCorrection]}
            </span>
          </div>
          {/* More correction survives more smudging, at more modules for
              the same data, so a busier symbol at the same size. */}
          <div className="flex items-center gap-1.5">
            {LEVELS.map((level) => (
              <Button
                key={level}
                size="icon"
                variant={
                  code.errorCorrection === level ? "default" : "secondary"
                }
                aria-label={`Error correction ${level.toUpperCase()}`}
                aria-pressed={code.errorCorrection === level}
                title={`${RECOVERY[level]} of the symbol recoverable`}
                onClick={() => set("errorCorrection", level)}
              >
                {level.toUpperCase()}
              </Button>
            ))}
          </div>
        </Field>
      </div>
    </FieldGroup>
  );
}
