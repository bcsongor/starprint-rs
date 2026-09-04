import { OptionSelect, type Option } from "@/components/option-select";
import { Field, FieldLabel } from "@/components/ui/field";
import type { Paper, Printer, Speed } from "@/lib/api";

const PAPERS: Option<Paper>[] = [
  { value: "80", label: "80 mm", hint: "576 dots" },
  { value: "112", label: "112 mm", hint: "832 dots" },
];

const DENSITIES: Option<string>[] = [3, 2, 1, 0, -1, -2, -3].map((value) => ({
  value: String(value),
  label: value > 0 ? `+${value}` : String(value),
  hint:
    value === 3
      ? "darkest"
      : value === 0
        ? "default"
        : value === -3
          ? "lightest"
          : "",
}));

const SPEEDS: Option<Speed>[] = [
  { value: "slow", label: "Slow", hint: "best quality" },
  { value: "medium", label: "Medium", hint: "" },
  { value: "high", label: "High", hint: "default" },
];

interface Props {
  printer: Printer;
  onChange: (printer: Printer) => void;
}

/** The printer's paper, density and speed: settings that apply to every
 * workflow. Thermal only; kept in place and disabled for impact so the
 * toolbar does not reflow when profiles change. */
export function PrintOptions({ printer, onChange }: Props) {
  const set = <K extends keyof Printer>(key: K, value: Printer[K]) =>
    onChange({ ...printer, [key]: value });
  const thermal = printer.kind === "thermal";

  return (
    <>
      <Field data-disabled={!thermal}>
        <FieldLabel htmlFor="paper">Paper</FieldLabel>
        <OptionSelect
          id="paper"
          value={printer.paper}
          options={PAPERS}
          disabled={!thermal}
          labelClassName="w-14"
          onChange={(paper) => set("paper", paper)}
        />
      </Field>

      <Field data-disabled={!thermal}>
        <FieldLabel htmlFor="density">Density</FieldLabel>
        <OptionSelect
          id="density"
          value={String(printer.density)}
          options={DENSITIES}
          disabled={!thermal}
          labelClassName="w-6 tabular-nums"
          onChange={(density) => set("density", Number(density))}
        />
      </Field>

      <Field data-disabled={!thermal}>
        <FieldLabel htmlFor="speed">Speed</FieldLabel>
        <OptionSelect
          id="speed"
          value={printer.speed}
          options={SPEEDS}
          disabled={!thermal}
          labelClassName="w-14"
          onChange={(speed) => set("speed", speed)}
        />
      </Field>
    </>
  );
}
