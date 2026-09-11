import { useRef } from "react";
import { FolderOpenIcon, RotateCcwIcon, XIcon } from "lucide-react";
import { OptionSelect, type Option } from "@/components/option-select";
import { SliderField } from "@/components/slider-field";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";
import { DEFAULT_PICTURE, type Dither, type Picture } from "@/lib/api";

const DITHERS: Option<Dither>[] = [
  { value: "floyd-steinberg", label: "Floyd–Steinberg", hint: "photos" },
  { value: "atkinson", label: "Atkinson", hint: "lighter, crisper" },
  { value: "threshold", label: "Threshold", hint: "line art" },
  { value: "bayer", label: "Bayer 8×8", hint: "ordered pattern" },
];

/** What the server decodes. */
const ACCEPT = "image/png,image/jpeg,image/webp,image/bmp";

interface Props {
  picture: Picture;
  file: File | null;
  thermal: boolean;
  /** Two-colour mode has one resolution, so the switch is off there. */
  twoColor: boolean;
  onChange: (picture: Picture) => void;
  onFile: (file: File | null) => void;
}

export function PictureForm({
  picture,
  file,
  thermal,
  twoColor,
  onChange,
  onFile,
}: Props) {
  const input = useRef<HTMLInputElement>(null);
  const set = <K extends keyof Picture>(key: K, value: Picture[K]) =>
    onChange({ ...picture, [key]: value });
  const hasThreshold = picture.dither !== "bayer";
  const adjusted = (Object.keys(DEFAULT_PICTURE) as (keyof Picture)[]).some(
    (key) => picture[key] !== DEFAULT_PICTURE[key],
  );

  return (
    <FieldGroup className="gap-4">
      <Field>
        {/* Same row height as the Task label, so the tabs do not shift. */}
        <div className="flex h-5 items-center">
          <FieldLabel htmlFor="browse">Picture</FieldLabel>
        </div>
        <div className="flex items-center gap-2">
          <input
            ref={input}
            type="file"
            accept={ACCEPT}
            className="hidden"
            onChange={(e) => {
              onFile(e.target.files?.[0] ?? null);
              // So the same file can be chosen again after Clear.
              e.target.value = "";
            }}
          />
          <Button
            id="browse"
            variant="outline"
            onClick={() => input.current?.click()}
          >
            <FolderOpenIcon />
            Browse…
          </Button>
          <span
            className="min-w-0 truncate text-sm text-muted-foreground"
            title={file?.name}
          >
            {file?.name ?? "No picture chosen."}
          </span>
          {file && (
            <Button
              variant="ghost"
              size="icon"
              aria-label="Clear the picture"
              title="Clear the picture"
              onClick={() => onFile(null)}
            >
              <XIcon />
            </Button>
          )}
        </div>
      </Field>

      {/* The switch is boxed to the select's height so the two line up
          on their bottom edge. */}
      <div className="flex items-end gap-4">
        <Field className="w-44">
          <FieldLabel htmlFor="dither">Dither</FieldLabel>
          <OptionSelect
            id="dither"
            value={picture.dither}
            options={DITHERS}
            align="start"
            labelClassName="w-30"
            onChange={(dither) => set("dither", dither)}
          />
        </Field>

        <Field
          orientation="horizontal"
          className="h-8 w-auto"
          data-disabled={twoColor}
          title={
            twoColor
              ? "Double resolution is unavailable at density +4."
              : undefined
          }
        >
          <Switch
            id="double"
            checked={picture.double && !twoColor}
            disabled={twoColor}
            onCheckedChange={(checked) => set("double", checked)}
          />
          <FieldLabel
            htmlFor="double"
            className="text-sm tracking-normal normal-case text-foreground"
          >
            {thermal ? "Double resolution" : "Double density"}
          </FieldLabel>
        </Field>

        <Button
          variant="ghost"
          size="icon"
          className="ml-auto"
          disabled={!adjusted}
          aria-label="Reset the settings"
          title="Reset the settings"
          onClick={() => onChange(DEFAULT_PICTURE)}
        >
          <RotateCcwIcon />
        </Button>
      </div>

      <SliderField
        id="threshold"
        label="Threshold"
        value={picture.threshold}
        display={String(picture.threshold)}
        min={1}
        max={255}
        disabled={!hasThreshold}
        onChange={(value) => set("threshold", value)}
      />
      <SliderField
        id="brightness"
        label="Brightness"
        value={Math.round(picture.brightness * 100)}
        display={picture.brightness.toFixed(2)}
        min={10}
        max={300}
        onChange={(value) => set("brightness", value / 100)}
      />
      <SliderField
        id="contrast"
        label="Contrast"
        value={Math.round(picture.contrast * 100)}
        display={picture.contrast.toFixed(2)}
        min={10}
        max={300}
        onChange={(value) => set("contrast", value / 100)}
      />
    </FieldGroup>
  );
}
