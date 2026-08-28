import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
  label:
    value === 0
      ? "0 (default)"
      : `${value > 0 ? "+" : ""}${value}${value === 3 ? " (darkest)" : value === -3 ? " (lightest)" : ""}`,
}));

const SPEEDS: { value: Speed; label: string }[] = [
  { value: "slow", label: "Slow (best quality)" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High (default)" },
];

function kindLabel(kind: PrinterKind) {
  const entry = KINDS.find((k) => k.value === kind) ?? KINDS[0];
  return `${entry.label} (${entry.models})`;
}

interface Props {
  printer: Printer;
  onChange: (printer: Printer) => void;
}

export function PrinterSettings({ printer, onChange }: Props) {
  const set = <K extends keyof Printer>(key: K, value: Printer[K]) =>
    onChange({ ...printer, [key]: value });

  return (
    <Card>
      <CardHeader>
        <CardTitle>Printer</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="grid gap-2">
          <Label htmlFor="kind">Type</Label>
          <Select
            value={printer.kind}
            onValueChange={(value) => set("kind", value as PrinterKind)}
          >
            <SelectTrigger id="kind" className="w-full">
              <SelectValue>{kindLabel(printer.kind)}</SelectValue>
            </SelectTrigger>
            <SelectContent>
              {KINDS.map((k) => (
                <SelectItem key={k.value} value={k.value}>
                  <span className="grid">
                    <span>{k.label}</span>
                    <span className="text-muted-foreground text-xs">
                      {k.models}
                    </span>
                  </span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="grid grid-cols-[1fr_6rem] gap-3">
          <div className="grid gap-2">
            <Label htmlFor="host">Host</Label>
            <Input
              id="host"
              value={printer.host}
              placeholder="192.168.1.60"
              spellCheck={false}
              autoComplete="off"
              aria-invalid={printer.host.trim() === "" || undefined}
              onChange={(e) => set("host", e.target.value)}
            />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="port">Port</Label>
            <Input
              id="port"
              type="number"
              min={1}
              max={65535}
              value={printer.port}
              onChange={(e) => set("port", Number(e.target.value) || 9100)}
            />
          </div>
        </div>

        {printer.kind === "thermal" && (
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-2">
              <Label htmlFor="density">Density</Label>
              <Select
                value={String(printer.density)}
                onValueChange={(value) => set("density", Number(value))}
              >
                <SelectTrigger id="density" className="w-full">
                  <SelectValue>
                    {DENSITIES.find((d) => d.value === printer.density)?.label}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {DENSITIES.map((d) => (
                    <SelectItem key={d.value} value={String(d.value)}>
                      {d.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-2">
              <Label htmlFor="speed">Speed</Label>
              <Select
                value={printer.speed}
                onValueChange={(value) => set("speed", value as Speed)}
              >
                <SelectTrigger id="speed" className="w-full">
                  <SelectValue>
                    {SPEEDS.find((s) => s.value === printer.speed)?.label}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent>
                  {SPEEDS.map((s) => (
                    <SelectItem key={s.value} value={s.value}>
                      {s.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <p className="col-span-2 text-muted-foreground text-xs">
              Slow speed and +2/+3 density stop cheap paper pinholing in solid
              black.
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
