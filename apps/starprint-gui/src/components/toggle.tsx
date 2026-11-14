import { Field, FieldLabel } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";

interface Props
  extends Omit<React.ComponentProps<typeof Field>, "id" | "orientation"> {
  id: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
}

/**
 * A switch with its label beside it, set as body text and not as a
 * field's small capitals. Anything else goes to the field around them.
 */
export function Toggle({
  id,
  checked,
  disabled,
  onCheckedChange,
  children,
  ...field
}: Props) {
  return (
    <Field orientation="horizontal" {...field}>
      <Switch
        id={id}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onCheckedChange}
      />
      <FieldLabel
        htmlFor={id}
        className="text-sm tracking-normal normal-case text-foreground"
      >
        {children}
      </FieldLabel>
    </Field>
  );
}
