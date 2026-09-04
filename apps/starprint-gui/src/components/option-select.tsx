import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

export interface Option<V extends string> {
  value: V;
  label: string;
  /** Shown beside the label in the list, greyed. */
  hint?: string;
}

interface Props<V extends string> {
  id: string;
  value: V;
  options: Option<V>[];
  onChange: (value: V) => void;
  disabled?: boolean;
  align?: "start" | "end";
  /** Width of the label column, so the hints line up. */
  labelClassName: string;
}

/** A select over a fixed list, showing the chosen label in the trigger. */
export function OptionSelect<V extends string>({
  id,
  value,
  options,
  onChange,
  disabled,
  align = "end",
  labelClassName,
}: Props<V>) {
  return (
    <Select
      modal={false}
      value={value}
      disabled={disabled}
      onValueChange={(next) => {
        if (next !== null) onChange(next);
      }}
    >
      <SelectTrigger id={id} className="w-full">
        <SelectValue>
          {options.find((o) => o.value === value)?.label}
        </SelectValue>
      </SelectTrigger>
      <SelectContent
        alignItemWithTrigger={false}
        align={align}
        className="w-max min-w-(--anchor-width)"
      >
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            <span className={labelClassName}>{o.label}</span>
            {o.hint && <span className="text-muted-foreground">{o.hint}</span>}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
