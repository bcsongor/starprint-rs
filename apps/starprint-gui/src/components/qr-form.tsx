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
  MAX_QR_RADIUS,
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

      {/* The sliders share one row, the two button groups the next. */}
      <div className="flex items-end gap-3">
        <SliderField
          id="size"
          label="Size"
          value={code.size}
          display={`${code.size} mm`}
          min={MIN_QR_MM}
          max={MAX_QR_MM}
          className="min-w-0 flex-1"
          onChange={(value) => set("size", value)}
        />

        {/* Full draws a lone module as a circle. A head with too few
            dots to a module prints square regardless, as the preview
            shows. */}
        <SliderField
          id="radius"
          label="Corners"
          value={code.radius}
          display={code.radius === 0 ? "Square" : `${code.radius}%`}
          min={0}
          max={MAX_QR_RADIUS}
          className="min-w-0 flex-1"
          onChange={(value) => set("radius", value)}
        />
      </div>

      <div className="flex items-end gap-3">
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
