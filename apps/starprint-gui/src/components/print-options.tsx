import { Field, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Printer, Speed } from "@/lib/api";

const DENSITIES = [3, 2, 1, 0, -1, -2, -3].map((value) => ({
  value,
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

const SPEEDS: { value: Speed; label: string; hint: string }[] = [
  { value: "slow", label: "Slow", hint: "best quality" },
  { value: "medium", label: "Medium", hint: "" },
  { value: "high", label: "High", hint: "default" },
];

interface Props {
  printer: Printer;
  onChange: (printer: Printer) => void;
}

/** Per-job density and speed. Thermal only; kept in place and disabled
 * for impact so the toolbar does not reflow when profiles change. */
export function PrintOptions({ printer, onChange }: Props) {
  const set = <K extends keyof Printer>(key: K, value: Printer[K]) =>
    onChange({ ...printer, [key]: value });
  const thermal = printer.kind === "thermal";

  return (
    <>
      <Field className="ml-auto w-24" data-disabled={!thermal}>
        <FieldLabel htmlFor="density">Density</FieldLabel>
        <Select
          value={String(printer.density)}
          disabled={!thermal}
          onValueChange={(value) => set("density", Number(value))}
        >
          <SelectTrigger id="density" className="w-full">
            <SelectValue>
              {DENSITIES.find((d) => d.value === printer.density)?.label}
            </SelectValue>
          </SelectTrigger>
          <SelectContent
            alignItemWithTrigger={false}
            align="end"
            className="w-max min-w-(--anchor-width)"
          >
            {DENSITIES.map((d) => (
              <SelectItem key={d.value} value={String(d.value)}>
                <span className="w-6 tabular-nums">{d.label}</span>
                {d.hint && (
                  <span className="text-muted-foreground">{d.hint}</span>
                )}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>

      <Field className="w-32" data-disabled={!thermal}>
        <FieldLabel htmlFor="speed">Speed</FieldLabel>
        <Select
          value={printer.speed}
          disabled={!thermal}
          onValueChange={(value) => set("speed", value as Speed)}
        >
          <SelectTrigger id="speed" className="w-full">
            <SelectValue>
              {SPEEDS.find((s) => s.value === printer.speed)?.label}
            </SelectValue>
          </SelectTrigger>
          <SelectContent
            alignItemWithTrigger={false}
            align="end"
            className="w-max min-w-(--anchor-width)"
          >
            {SPEEDS.map((s) => (
              <SelectItem key={s.value} value={s.value}>
                <span className="w-14">{s.label}</span>
                {s.hint && (
                  <span className="text-muted-foreground">{s.hint}</span>
                )}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>
    </>
  );
}
