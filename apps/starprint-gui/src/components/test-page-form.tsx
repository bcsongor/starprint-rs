import { Toggle } from "@/components/toggle";
import { FieldGroup } from "@/components/ui/field";
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

      <Toggle
        id="double-resolution"
        data-disabled={!thermal}
        checked={page.doubleResolution}
        disabled={!thermal}
        onCheckedChange={(checked) => onChange({ doubleResolution: checked })}
      >
        Repeat the grey ramp in double resolution
      </Toggle>
    </FieldGroup>
  );
}
