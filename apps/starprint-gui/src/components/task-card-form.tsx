import { addDays, format, parseISO } from "date-fns";
import { CalendarIcon } from "lucide-react";
import { SubmitTextarea } from "@/components/submit-textarea";
import { Toggle } from "@/components/toggle";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { TaskCard } from "@/lib/api";

interface Props {
  card: TaskCard;
  onChange: (card: TaskCard) => void;
  /** Called on Ctrl/Cmd+Enter in the task field. */
  onSubmit: () => void;
}

const ISO = "yyyy-MM-dd";

function toDate(due: string | null): Date | undefined {
  if (!due) return undefined;
  const date = parseISO(due);
  return Number.isNaN(date.getTime()) ? undefined : date;
}

export function TaskCardForm({ card, onChange, onSubmit }: Props) {
  const set = <K extends keyof TaskCard>(key: K, value: TaskCard[K]) =>
    onChange({ ...card, [key]: value });
  const setDate = (date: Date | undefined) =>
    set("due", date ? format(date, ISO) : null);

  const today = new Date();
  const selected = toDate(card.due);
  const dayAfter = addDays(today, 2);
  const shortcuts: [string, Date][] = [
    ["Today", today],
    ["Tomorrow", addDays(today, 1)],
    [format(dayAfter, "EEEE"), dayAfter],
  ];

  return (
    <FieldGroup className="gap-4">
      <SubmitTextarea
        id="task"
        label="Task"
        // 5px padding makes an empty box the date button's 32px height.
        className="min-h-8 border-border bg-background py-[0.3125rem] text-sm"
        value={card.text}
        placeholder="What needs doing?"
        rows={3}
        onChange={(text) => set("text", text)}
        onSubmit={onSubmit}
      />

      <Field>
        <FieldLabel htmlFor="due">Due</FieldLabel>
        {/* Pressing the active shortcut or selected day unsets the date. */}
        <div className="flex items-center gap-2">
          <Popover>
            <PopoverTrigger
              render={
                <Button
                  id="due"
                  variant="outline"
                  className="w-34 justify-start font-normal"
                />
              }
            >
              <CalendarIcon />
              {selected ? (
                format(selected, "d MMM yyyy")
              ) : (
                <span className="text-muted-foreground">No due date</span>
              )}
            </PopoverTrigger>
            <PopoverContent className="w-auto p-0" align="start">
              <Calendar
                selected={selected}
                defaultMonth={selected}
                onSelect={setDate}
                weekStartsOn={1}
              />
            </PopoverContent>
          </Popover>
          {shortcuts.map(([label, date]) => {
            const active = format(date, ISO) === card.due;
            return (
              <Button
                key={label}
                variant={active ? "default" : "secondary"}
                className="px-2.5"
                aria-pressed={active}
                onClick={() => setDate(active ? undefined : date)}
              >
                {label}
              </Button>
            );
          })}
        </div>
      </Field>

      <Toggle
        id="priority"
        checked={card.priority}
        onCheckedChange={(checked) => set("priority", checked)}
      >
        High priority
      </Toggle>
    </FieldGroup>
  );
}
