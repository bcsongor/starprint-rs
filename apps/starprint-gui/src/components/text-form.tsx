import {
  BoldIcon,
  ContrastIcon,
  CornerDownLeftIcon,
  DropletIcon,
  UnfoldHorizontalIcon,
  UnfoldVerticalIcon,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Textarea } from "@/components/ui/textarea";
import type { PrinterKind, Text } from "@/lib/api";

type Style = "bold" | "wide" | "tall" | "accent";

interface Props {
  text: Text;
  kind: PrinterKind;
  onChange: (text: Text) => void;
  /** Called on Ctrl/Cmd+Enter in the text field. */
  onSubmit: () => void;
}

export function TextForm({ text, kind, onChange, onSubmit }: Props) {
  const set = <K extends keyof Text>(key: K, value: Text[K]) =>
    onChange({ ...text, [key]: value });

  const styles: { key: Style; label: string; Icon: LucideIcon }[] = [
    { key: "bold", label: "Bold", Icon: BoldIcon },
    { key: "wide", label: "Wide", Icon: UnfoldHorizontalIcon },
    { key: "tall", label: "Tall", Icon: UnfoldVerticalIcon },
    // A thermal head has no second colour, so it inverts instead.
    kind === "impact"
      ? { key: "accent", label: "Red", Icon: DropletIcon }
      : { key: "accent", label: "Inverse", Icon: ContrastIcon },
  ];

  return (
    <FieldGroup className="gap-4">
      <Field>
        <div className="flex h-5 items-center justify-between">
          <FieldLabel htmlFor="text">Text</FieldLabel>
          <span className="flex items-center gap-1 text-xs text-muted-foreground">
            <KbdGroup>
              <Kbd>Ctrl</Kbd>
              <Kbd>
                <CornerDownLeftIcon />
              </Kbd>
            </KbdGroup>
            prints
          </span>
        </div>
        <Textarea
          id="text"
          // Grows with the text up to what the shortest window fits.
          className="max-h-48 min-h-28 border-border bg-background text-sm"
          value={text.text}
          placeholder="Anything you want on paper"
          autoFocus
          onChange={(e) => set("text", e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              e.preventDefault();
              onSubmit();
            }
          }}
        />
      </Field>

      <Field>
        <FieldLabel render={<span />}>Style</FieldLabel>
        {/* Buttons, not toggles: a toggle's tint is too quiet to read. */}
        <div className="flex items-center gap-2">
          {styles.map(({ key, label, Icon }) => (
            <Button
              key={key}
              variant={text[key] ? "default" : "secondary"}
              className="px-2.5"
              aria-pressed={text[key]}
              onClick={() => set(key, !text[key])}
            >
              <Icon />
              {label}
            </Button>
          ))}
        </div>
      </Field>
    </FieldGroup>
  );
}
