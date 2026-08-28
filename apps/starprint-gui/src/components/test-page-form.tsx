import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";
import type { PrinterKind, TestPage } from "@/lib/api";

interface Props {
  page: TestPage;
  kind: PrinterKind;
  onChange: (page: TestPage) => void;
}

/** Options for the head-check page. Only thermal printers have any. */
export function TestPageForm({ page, kind, onChange }: Props) {
  const thermal = kind === "thermal";

  return (
    <FieldGroup className="gap-4">
      {/* Same row height as the Task label, so the tabs do not shift. */}
      <div className="h-5" />

      <Field orientation="horizontal" data-disabled={!thermal}>
        <Switch
          id="double-resolution"
          checked={page.doubleResolution}
          disabled={!thermal}
          onCheckedChange={(checked) => onChange({ doubleResolution: checked })}
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
