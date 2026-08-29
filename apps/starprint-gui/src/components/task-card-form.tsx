import { addDays, format, nextSunday, parseISO } from "date-fns";
import { CalendarIcon, CornerDownLeftIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Kbd, KbdGroup } from "@/components/ui/kbd";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
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
  const shortcuts: [string, Date][] = [
    ["Today", today],
    ["Tomorrow", addDays(today, 1)],
    ["Sunday", nextSunday(today)],
  ];

  return (
    <FieldGroup className="gap-4">
      <Field>
        <div className="flex h-5 items-center justify-between">
          <FieldLabel htmlFor="task">Task</FieldLabel>
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
          id="task"
          // 5px padding makes an empty box the date button's 32px height.
          className="min-h-0 border-border bg-background py-[0.3125rem] text-sm"
          value={card.text}
          placeholder="What needs doing?"
          rows={3}
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
                mode="single"
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

      <Field orientation="horizontal">
        <Switch
          id="priority"
          checked={card.priority}
          onCheckedChange={(checked) => set("priority", checked)}
        />
        <FieldLabel
          htmlFor="priority"
          className="text-sm tracking-normal normal-case text-foreground"
        >
          High priority
        </FieldLabel>
      </Field>
    </FieldGroup>
  );
}
