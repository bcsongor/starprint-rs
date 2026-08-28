import { addDays, format, nextSaturday, nextSunday, parseISO } from "date-fns";
import { CalendarIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
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

const LABEL =
  "text-[11px] font-medium uppercase tracking-wider text-muted-foreground";

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
    ["Sat", nextSaturday(today)],
    ["Sun", nextSunday(today)],
  ];

  return (
    <div className="grid gap-3">
      <div className="grid gap-1">
        <div className="flex items-baseline justify-between">
          <label htmlFor="task" className={LABEL}>
            Task
          </label>
          <span className="text-[11px] text-muted-foreground">
            Ctrl+Enter prints
          </span>
        </div>
        <Textarea
          id="task"
          className="min-h-0 text-[13px] leading-snug"
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
      </div>

      <div className="grid gap-1">
        <label className={LABEL}>Due</label>
        <div className="flex flex-wrap items-center gap-1.5">
          <Popover>
            <PopoverTrigger
              render={
                <Button
                  variant="outline"
                  size="sm"
                  className="w-44 justify-start font-normal"
                />
              }
            >
              <CalendarIcon />
              {selected ? (
                format(selected, "EEE d MMM yyyy")
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
                size="xs"
                aria-pressed={active}
                onClick={() => setDate(active ? undefined : date)}
              >
                {label}
              </Button>
            );
          })}
          {selected && (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Clear due date"
              onClick={() => setDate(undefined)}
            >
              <XIcon />
            </Button>
          )}
        </div>
      </div>

      <label className="flex items-center gap-2">
        <Switch
          id="priority"
          size="sm"
          checked={card.priority}
          onCheckedChange={(checked) => set("priority", checked)}
        />
        <span>High priority</span>
        <span className="text-muted-foreground">
          — prints a flag, red on impact, inverse on thermal
        </span>
      </label>
    </div>
  );
}
