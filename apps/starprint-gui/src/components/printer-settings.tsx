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
import { Switch } from "@/components/ui/switch";
import type { Printer, PrinterKind } from "@/lib/api";

const DENSITIES = [-3, -2, -1, 0, 1, 2, 3];

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
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="thermal">Thermal (TSP800II)</SelectItem>
              <SelectItem value="impact">Impact (SP700)</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div className="grid grid-cols-[1fr_6rem] gap-2">
          <div className="grid gap-2">
            <Label htmlFor="host">Host</Label>
            <Input
              id="host"
              value={printer.host}
              placeholder="192.168.1.60"
              spellCheck={false}
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
          <>
            <div className="grid gap-2">
              <Label htmlFor="density">Density</Label>
              <Select
                value={String(printer.density)}
                onValueChange={(value) => set("density", Number(value))}
              >
                <SelectTrigger id="density" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {DENSITIES.map((d) => (
                    <SelectItem key={d} value={String(d)}>
                      {d > 0 ? `+${d}` : d}
                      {d === 0 ? " (default)" : ""}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="slow">Slow speed</Label>
              <Switch
                id="slow"
                checked={printer.slow}
                onCheckedChange={(checked) => set("slow", checked)}
              />
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
