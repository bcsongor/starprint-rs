import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Printer, PrinterKind, Speed } from "@/lib/api";

const KINDS: { value: PrinterKind; label: string; models: string }[] = [
  {
    value: "thermal",
    label: "Thermal",
    models: "TSP650II, TSP700II, TSP800II",
  },
  { value: "impact", label: "Impact", models: "SP712, SP742, SP717, SP747" },
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

export function PrinterSettings({ printer, onChange }: Props) {
  const set = <K extends keyof Printer>(key: K, value: Printer[K]) =>
    onChange({ ...printer, [key]: value });
  const kind = KINDS.find((k) => k.value === printer.kind) ?? KINDS[0];
  const thermal = printer.kind === "thermal";

  return (
    <div className="flex flex-wrap items-end gap-x-3 gap-y-2">
      <Field className="min-w-64 flex-1">
        <FieldLabel htmlFor="kind">Printer</FieldLabel>
        <Select
          value={printer.kind}
          onValueChange={(value) => set("kind", value as PrinterKind)}
        >
          <SelectTrigger id="kind" className="w-full">
            <SelectValue>
              {kind.label}
              <span className="text-muted-foreground"> · {kind.models}</span>
            </SelectValue>
          </SelectTrigger>
          <SelectContent alignItemWithTrigger={false} align="start">
            {KINDS.map((k) => (
              <SelectItem key={k.value} value={k.value}>
                {k.label}
                <span className="text-muted-foreground"> · {k.models}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>

      <Field className="w-36">
        <FieldLabel htmlFor="host">Host</FieldLabel>
        <Input
          id="host"
          value={printer.host}
          placeholder="192.168.1.60"
          spellCheck={false}
          autoComplete="off"
          aria-invalid={printer.host.trim() === "" || undefined}
          onChange={(e) => set("host", e.target.value)}
        />
      </Field>

      <Field className="w-18">
        <FieldLabel htmlFor="port">Port</FieldLabel>
        <Input
          id="port"
          type="number"
          min={1}
          max={65535}
          value={printer.port}
          onChange={(e) => set("port", Number(e.target.value) || 9100)}
        />
      </Field>

      {/* Density and speed are thermal-only; they stay in place, disabled,
          for the impact printer so the toolbar does not reflow. */}
      <Field className="w-20" data-disabled={!thermal}>
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

      <Field className="w-26" data-disabled={!thermal}>
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
    </div>
  );
}
