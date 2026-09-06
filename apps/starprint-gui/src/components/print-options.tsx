import { OptionSelect, type Option } from "@/components/option-select";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  TWO_COLOR_DENSITY,
  type Paper,
  type Printer,
  type Speed,
} from "@/lib/api";
import { roll } from "@/lib/paper";

const PAPERS: Option<Paper>[] = [
  { value: "80", label: "80 mm", hint: "576 dots" },
  { value: "112", label: "112 mm", hint: "832 dots" },
];

const DENSITIES: Option<string>[] = [
  { value: String(TWO_COLOR_DENSITY), label: "+4", hint: "two-colour" },
  { value: "3", label: "+3", hint: "darkest" },
  { value: "2", label: "+2" },
  { value: "1", label: "+1" },
  { value: "0", label: "0", hint: "default" },
  { value: "-1", label: "-1" },
  { value: "-2", label: "-2" },
  { value: "-3", label: "-3", hint: "lightest" },
];

const SPEEDS: Option<Speed>[] = [
  { value: "slow", label: "Slow", hint: "best quality" },
  { value: "medium", label: "Medium", hint: "" },
  { value: "high", label: "High", hint: "default" },
];

interface Props {
  printer: Printer;
  onChange: (printer: Printer) => void;
}

/** Paper width, with density and speed controls for thermal printers. */
export function PrintOptions({ printer, onChange }: Props) {
  const set = <K extends keyof Printer>(key: K, value: Printer[K]) =>
    onChange({ ...printer, [key]: value });
  const thermal = printer.kind === "thermal";
  const twoColor = thermal && printer.density === TWO_COLOR_DENSITY;

  return (
    <>
      <Field data-disabled={!thermal}>
        <FieldLabel htmlFor="paper">Paper</FieldLabel>
        {thermal ? (
          <OptionSelect
            id="paper"
            value={printer.paper}
            options={PAPERS}
            labelClassName="w-14"
            onChange={(paper) => set("paper", paper)}
          />
        ) : (
          <Input
            id="paper"
            value={`${roll(printer.kind, printer.paper).paperMm} mm`}
            disabled
          />
        )}
      </Field>

      {thermal && (
        <>
          <Field>
            <FieldLabel htmlFor="density">Density</FieldLabel>
            <OptionSelect
              id="density"
              value={String(printer.density)}
              options={DENSITIES}
              labelClassName="w-6 text-right tabular-nums"
              onChange={(density) => set("density", Number(density))}
            />
          </Field>

          <Field
            data-disabled={twoColor}
            title={twoColor ? "Two-colour mode has one speed." : undefined}
          >
            <FieldLabel htmlFor="speed">Speed</FieldLabel>
            <OptionSelect
              id="speed"
              value={printer.speed}
              options={SPEEDS}
              disabled={twoColor}
              labelClassName="w-14"
              onChange={(speed) => set("speed", speed)}
            />
          </Field>
        </>
      )}
    </>
  );
}
