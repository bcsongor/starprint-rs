import { Field, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Paper, Printer, Speed } from "@/lib/api";

const PAPERS: { value: Paper; label: string; hint: string }[] = [
  { value: "80", label: "80 mm", hint: "576 dots" },
  { value: "112", label: "112 mm", hint: "832 dots" },
];

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
        <Select
          modal={false}
          value={printer.paper}
          disabled={!thermal}
          onValueChange={(value) => set("paper", value as Paper)}
        >
          <SelectTrigger id="paper" className="w-full">
            <SelectValue>
              {PAPERS.find((p) => p.value === printer.paper)?.label}
            </SelectValue>
          </SelectTrigger>
          <SelectContent
            alignItemWithTrigger={false}
            align="end"
            className="w-max min-w-(--anchor-width)"
          >
            {PAPERS.map((p) => (
              <SelectItem key={p.value} value={p.value}>
                <span className="w-14">{p.label}</span>
                <span className="text-muted-foreground">{p.hint}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>

      <Field data-disabled={!thermal}>
        <FieldLabel htmlFor="density">Density</FieldLabel>
        <Select
          modal={false}
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

      <Field data-disabled={!thermal}>
        <FieldLabel htmlFor="speed">Speed</FieldLabel>
        <Select
          modal={false}
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
