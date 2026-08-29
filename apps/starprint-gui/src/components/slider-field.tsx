import { Field, FieldLabel } from "@/components/ui/field";
import { Slider } from "@/components/ui/slider";

interface Props {
  id: string;
  label: string;
  value: number;
  /** Shown against the label, in the value's own units. */
  display: string;
  min: number;
  max: number;
  disabled?: boolean;
  onChange: (value: number) => void;
}

export function SliderField({
  id,
  label,
  value,
  display,
  min,
  max,
  disabled,
  onChange,
}: Props) {
  return (
    <Field data-disabled={disabled}>
      <div className="flex items-center justify-between">
        <FieldLabel htmlFor={id}>{label}</FieldLabel>
        <span className="font-mono text-xs tabular-nums text-muted-foreground">
          {display}
        </span>
      </div>
      <Slider
        id={id}
        value={[value]}
        min={min}
        max={max}
        step={1}
        disabled={disabled}
        onValueChange={(next) => onChange(Array.isArray(next) ? next[0] : next)}
      />
    </Field>
  );
}
