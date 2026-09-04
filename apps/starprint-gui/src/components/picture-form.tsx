import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpenIcon, RotateCcwIcon, XIcon } from "lucide-react";
import { SliderField } from "@/components/slider-field";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  DEFAULT_PICTURE,
  type Dither,
  type Picture,
  type PrinterKind,
} from "@/lib/api";

const DITHERS: { value: Dither; label: string; hint: string }[] = [
  { value: "floyd-steinberg", label: "Floyd–Steinberg", hint: "photos" },
  { value: "atkinson", label: "Atkinson", hint: "lighter, crisper" },
  { value: "threshold", label: "Threshold", hint: "line art" },
  { value: "bayer", label: "Bayer 8×8", hint: "ordered pattern" },
];

const FILE_FILTERS = [
  {
    name: "Images",
    extensions: ["png", "jpg", "jpeg", "webp", "bmp"],
  },
];

interface Props {
  picture: Picture;
  kind: PrinterKind;
  onChange: (picture: Picture) => void;
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

export function PictureForm({ picture, kind, onChange }: Props) {
  const set = <K extends keyof Picture>(key: K, value: Picture[K]) =>
    onChange({ ...picture, [key]: value });
  const thermal = kind === "thermal";
  const hasThreshold = picture.dither !== "bayer";
  const adjusted = (Object.keys(DEFAULT_PICTURE) as (keyof Picture)[]).some(
    (key) => key !== "path" && picture[key] !== DEFAULT_PICTURE[key],
  );

  const browse = async () => {
    const path = await open({
      title: "Select a picture",
      multiple: false,
      directory: false,
      filters: FILE_FILTERS,
    });
    if (typeof path === "string") set("path", path);
  };

  return (
    <FieldGroup className="gap-4">
      <Field>
        {/* Same row height as the Task label, so the tabs do not shift. */}
        <div className="flex h-5 items-center">
          <FieldLabel htmlFor="browse">Picture</FieldLabel>
        </div>
        <div className="flex items-center gap-2">
          <Button id="browse" variant="outline" onClick={browse}>
            <FolderOpenIcon />
            Browse…
          </Button>
          <span
            className="min-w-0 truncate text-sm text-muted-foreground"
            title={picture.path}
          >
            {picture.path ? fileName(picture.path) : "No picture chosen."}
          </span>
          {picture.path && (
            <Button
              variant="ghost"
              size="icon"
              aria-label="Clear the picture"
              title="Clear the picture"
              onClick={() => set("path", "")}
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
          <Select
            modal={false}
            value={picture.dither}
            onValueChange={(value) => set("dither", value as Dither)}
          >
            <SelectTrigger id="dither" className="w-full">
              <SelectValue>
                {DITHERS.find((d) => d.value === picture.dither)?.label}
              </SelectValue>
            </SelectTrigger>
            <SelectContent
              alignItemWithTrigger={false}
              align="start"
              className="w-max min-w-(--anchor-width)"
            >
              {DITHERS.map((d) => (
                <SelectItem key={d.value} value={d.value}>
                  <span className="w-30">{d.label}</span>
                  <span className="text-muted-foreground">{d.hint}</span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>

        <Field orientation="horizontal" className="h-8 w-auto">
          <Switch
            id="double"
            checked={picture.double}
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
          onClick={() => onChange({ ...DEFAULT_PICTURE, path: picture.path })}
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
