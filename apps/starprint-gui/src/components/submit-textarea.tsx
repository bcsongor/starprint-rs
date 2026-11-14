import { CornerDownLeftIcon } from "lucide-react";
import { Field, FieldLabel } from "@/components/ui/field";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Textarea } from "@/components/ui/textarea";

interface Props
  extends Omit<
    React.ComponentProps<typeof Textarea>,
    "id" | "value" | "onChange" | "onKeyDown"
  > {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** Called on Ctrl/Cmd+Enter. */
  onSubmit: () => void;
}

/** The label of a field a job is typed into, with the shortcut that
 * prints it beside it. */
export function SubmitLabel({
  children,
  ...props
}: React.ComponentProps<typeof FieldLabel>) {
  return (
    <div className="flex h-5 items-center justify-between">
      <FieldLabel {...props}>{children}</FieldLabel>
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
  );
}

/** The field a job is typed into, which prints it on Ctrl/Cmd+Enter and
 * says so beside its label. */
export function SubmitTextarea({
  id,
  label,
  value,
  onChange,
  onSubmit,
  ...props
}: Props) {
  return (
    <Field>
      <SubmitLabel htmlFor={id}>{label}</SubmitLabel>
      <Textarea
        id={id}
        value={value}
        autoFocus
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            onSubmit();
          }
        }}
        {...props}
      />
    </Field>
  );
}
