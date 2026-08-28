import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Printer, PrinterKind, Speed } from "@/lib/api";
import { cn } from "@/lib/utils";

const KINDS: { value: PrinterKind; label: string; models: string }[] = [
  { value: "thermal", label: "Thermal", models: "TSP650II, TSP700II, TSP800II" },
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

/** A labelled cell in the toolbar. */
function Field({
  label,
  htmlFor,
  className,
  children,
}: {
  label: string;
  htmlFor: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div className={cn("grid gap-1", className)}>
      <label
        htmlFor={htmlFor}
        className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground"
      >
        {label}
      </label>
      {children}
    </div>
  );
}

const INPUT = "h-7 text-[0.8rem] px-2";

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
      <Field label="Printer" htmlFor="kind" className="min-w-56 flex-1">
        <Select
          value={printer.kind}
          onValueChange={(value) => set("kind", value as PrinterKind)}
        >
          <SelectTrigger id="kind" size="sm" className="w-full">
            <SelectValue>
              {kind.label}
              <span className="text-muted-foreground"> · {kind.models}</span>
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            {KINDS.map((k) => (
              <SelectItem key={k.value} value={k.value}>
                {k.label}
                <span className="text-muted-foreground"> · {k.models}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>

      <Field label="Host" htmlFor="host" className="w-36">
        <Input
          id="host"
          className={INPUT}
          value={printer.host}
          placeholder="192.168.1.60"
          spellCheck={false}
          autoComplete="off"
          aria-invalid={printer.host.trim() === "" || undefined}
          onChange={(e) => set("host", e.target.value)}
        />
      </Field>

      <Field label="Port" htmlFor="port" className="w-18">
        <Input
          id="port"
          className={INPUT}
          type="number"
          min={1}
          max={65535}
          value={printer.port}
          onChange={(e) => set("port", Number(e.target.value) || 9100)}
        />
      </Field>

      {thermal && (
        <>
          <Field label="Density" htmlFor="density" className="w-22">
            <Select
              value={String(printer.density)}
              onValueChange={(value) => set("density", Number(value))}
            >
              <SelectTrigger id="density" size="sm" className="w-full">
                <SelectValue>
                  {DENSITIES.find((d) => d.value === printer.density)?.label}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
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

          <Field label="Speed" htmlFor="speed" className="w-24">
            <Select
              value={printer.speed}
              onValueChange={(value) => set("speed", value as Speed)}
            >
              <SelectTrigger id="speed" size="sm" className="w-full">
                <SelectValue>
                  {SPEEDS.find((s) => s.value === printer.speed)?.label}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
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
      )}
    </div>
  );
}
