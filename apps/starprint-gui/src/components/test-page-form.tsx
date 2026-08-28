import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { Paper, PrinterKind, TestPage } from "@/lib/api";

const PAPERS: { value: Paper; label: string; hint: string }[] = [
  { value: "80", label: "80 mm", hint: "576 dots" },
  { value: "112", label: "112 mm", hint: "832 dots" },
];

interface Props {
  page: TestPage;
  kind: PrinterKind;
  onChange: (page: TestPage) => void;
}

/** Options for the head-check page. Only thermal printers have any. */
export function TestPageForm({ page, kind, onChange }: Props) {
  const set = <K extends keyof TestPage>(key: K, value: TestPage[K]) =>
    onChange({ ...page, [key]: value });
  const thermal = kind === "thermal";

  return (
    <FieldGroup className="gap-4">
      <Field className="w-40" data-disabled={!thermal}>
        {/* Same row height as the Task label, so the tabs do not shift. */}
        <div className="flex h-5 items-center">
          <FieldLabel htmlFor="paper">Paper</FieldLabel>
        </div>
        <Select
          value={page.paper}
          disabled={!thermal}
          onValueChange={(value) => set("paper", value as Paper)}
        >
          <SelectTrigger id="paper" className="w-full">
            <SelectValue>
              {PAPERS.find((p) => p.value === page.paper)?.label}
            </SelectValue>
          </SelectTrigger>
          <SelectContent alignItemWithTrigger={false} align="start">
            {PAPERS.map((p) => (
              <SelectItem key={p.value} value={p.value}>
                <span className="w-14">{p.label}</span>
                <span className="text-muted-foreground">{p.hint}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </Field>

      <Field orientation="horizontal" data-disabled={!thermal}>
        <Switch
          id="double-resolution"
          checked={page.doubleResolution}
          disabled={!thermal}
          onCheckedChange={(checked) => set("doubleResolution", checked)}
        />
        <FieldLabel
          htmlFor="double-resolution"
          className="text-sm tracking-normal normal-case text-foreground"
        >
          Repeat the grey ramp in double resolution
        </FieldLabel>
      </Field>
    </FieldGroup>
  );
}
