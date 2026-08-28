import { addDays, format, nextSaturday, nextSunday, parseISO } from "date-fns";
import { CalendarIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Label } from "@/components/ui/label";
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

export function TaskCardForm({ card, onChange, onSubmit }: Props) {
  const set = <K extends keyof TaskCard>(key: K, value: TaskCard[K]) =>
    onChange({ ...card, [key]: value });
  const setDate = (date: Date | undefined) =>
    set("due", date ? format(date, ISO) : null);

  const today = new Date();
  const selected = toDate(card.due);
  const selectedIso = card.due;
  const shortcuts: [string, Date][] = [
    ["Today", today],
    ["Tomorrow", addDays(today, 1)],
    ["Saturday", nextSaturday(today)],
    ["Sunday", nextSunday(today)],
  ];

  return (
    <div className="grid gap-5">
      <div className="grid gap-2">
        <div className="flex items-baseline justify-between">
          <Label htmlFor="task">Task</Label>
          <span className="text-muted-foreground text-xs">
            Ctrl+Enter to print
          </span>
        </div>
        <Textarea
          id="task"
          value={card.text}
          placeholder="What needs doing?"
          rows={4}
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

      <div className="flex items-center justify-between">
        <div className="grid gap-0.5">
          <Label htmlFor="priority">High priority</Label>
          <p className="text-muted-foreground text-xs">
            Prints a “HIGH PRIORITY” flag: red on impact, inverse on thermal.
          </p>
        </div>
        <Switch
          id="priority"
          checked={card.priority}
          onCheckedChange={(checked) => set("priority", checked)}
        />
      </div>

      <div className="grid gap-2">
        <Label>Due</Label>
        <div className="flex gap-2">
          <Popover>
            <PopoverTrigger
              render={
                <Button
                  variant="outline"
                  className="flex-1 justify-start font-normal"
                />
              }
            >
              <CalendarIcon />
              {selected ? (
                format(selected, "EEEE d MMMM yyyy")
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
          {selected && (
            <Button
              variant="ghost"
              size="icon"
              aria-label="Clear due date"
              onClick={() => setDate(undefined)}
            >
              <XIcon />
            </Button>
          )}
        </div>
        <div className="flex flex-wrap gap-1.5">
          {shortcuts.map(([label, date]) => {
            const active = format(date, ISO) === selectedIso;
            return (
              <Button
                key={label}
                variant={active ? "default" : "secondary"}
                size="sm"
                aria-pressed={active}
                onClick={() => setDate(active ? undefined : date)}
              >
                {label}
              </Button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
