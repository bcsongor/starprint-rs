import { useState } from "react";
import { OptionSelect, type Option } from "@/components/option-select";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Due, Profile, ScheduleSpec } from "@/lib/api";
import {
  NAMES,
  PRESETS,
  describe,
  formatNext,
  fromCron,
  nextRun,
  summary,
  toCron,
  type Preset,
} from "@/lib/schedule";

/** A schedule being written; `id` once it is on the server. */
export interface Draft extends ScheduleSpec {
  id?: string;
}

interface Props {
  /** The schedule to edit, or null when closed. */
  draft: Draft | null;
  profiles: Profile[];
  onClose: () => void;
  onSave: (draft: Draft) => void;
}

export function ScheduleDialog({ draft, profiles, onClose, onSave }: Props) {
  return (
    <Dialog
      open={draft !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      {/* Mounted per opening so the form starts from the draft. */}
      {draft && (
        <ScheduleForm
          initial={draft}
          profiles={profiles}
          onCancel={onClose}
          onSave={onSave}
        />
      )}
    </Dialog>
  );
}

const WHEN: Option<Preset | "custom">[] = [
  ...Object.entries(PRESETS).map(([value, { label }]) => ({
    value: value as Preset,
    label,
  })),
  { value: "custom", label: "Custom" },
];

const DUES: Option<Due | "none">[] = [
  { value: "run-day", label: "The day it prints" },
  { value: "next-day", label: "The next day" },
  { value: "none", label: "No due date" },
];

/** When the expression next fires, or what is wrong with it. */
function check(cron: string): { at: Date } | { error: string } {
  try {
    return { at: nextRun(cron) };
  } catch (error) {
    return { error: (error as Error).message };
  }
}

function ScheduleForm({
  initial,
  profiles,
  onCancel,
  onSave,
}: {
  initial: Draft;
  profiles: Profile[];
  onCancel: () => void;
  onSave: (draft: Draft) => void;
}) {
  const [schedule, setSchedule] = useState(initial);
  const editing = initial.id !== undefined;
  const set = <K extends keyof Draft>(key: K, value: Draft[K]) =>
    setSchedule({ ...schedule, [key]: value });

  // A preset and the time write the cron field; Custom opens it for typing.
  const { cron } = schedule;
  const spelled = fromCron(cron);
  const [custom, setCustom] = useState(spelled === null);
  const next = check(cron);
  const valid = "at" in next;
  const printer = profiles.find((p) => p.name === schedule.printer);

  return (
    <DialogContent className="gap-4 sm:max-w-md">
      <DialogHeader>
        <DialogTitle>
          {editing ? "Edit schedule" : `Schedule ${NAMES[schedule.job.kind]}`}
        </DialogTitle>
        <DialogDescription>
          {editing
            ? "Load into form to change the job itself."
            : "Print this job at the chosen times while the app is open."}
        </DialogDescription>
      </DialogHeader>

      <FieldGroup className="gap-4">
        <Field>
          <FieldLabel>Job</FieldLabel>
          <div className="flex h-8 items-center truncate rounded-lg border border-input bg-muted/50 px-2.5 text-sm">
            {summary(schedule.job)}
          </div>
        </Field>

        <Field>
          <FieldLabel htmlFor="schedule-printer">Printer</FieldLabel>
          <Select
            modal={false}
            value={schedule.printer}
            onValueChange={(name) => {
              if (name) set("printer", name);
            }}
          >
            <SelectTrigger id="schedule-printer" className="w-full">
              <SelectValue placeholder="No printer">{printer?.name}</SelectValue>
            </SelectTrigger>
            <SelectContent
              alignItemWithTrigger={false}
              align="start"
              className="w-max max-w-(--available-width) min-w-(--anchor-width)"
            >
              {profiles.map((p) => (
                <SelectItem key={p.name} value={p.name}>
                  {p.name}
                  <span className="text-muted-foreground"> · {p.host}</span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>

        <Field>
          <FieldLabel htmlFor="schedule-when">When</FieldLabel>
          <div className="flex items-center gap-2">
            <div className="flex-1">
              <OptionSelect
                id="schedule-when"
                value={custom ? "custom" : (spelled?.preset ?? "custom")}
                options={WHEN}
                align="start"
                labelClassName="w-52"
                onChange={(preset) => {
                  setCustom(preset === "custom");
                  if (preset !== "custom") {
                    set("cron", toCron(preset, spelled?.time ?? "09:00"));
                  }
                }}
              />
            </div>
            {/* Typed, not picked: the browser's clock button is hidden. */}
            <Input
              type="time"
              aria-label="Time"
              className="w-24 shrink-0 tabular-nums [&::-webkit-calendar-picker-indicator]:hidden"
              value={spelled?.time ?? ""}
              disabled={custom}
              onChange={(e) => {
                if (spelled)
                  set("cron", toCron(spelled.preset, e.target.value));
              }}
            />
          </div>
        </Field>

        <Field data-invalid={!valid}>
          <div className="flex h-5 items-center justify-between">
            <FieldLabel htmlFor="schedule-cron">Cron</FieldLabel>
            <span className="text-xs text-muted-foreground">
              minute hour day month weekday
            </span>
          </div>
          <Input
            id="schedule-cron"
            className="font-mono"
            spellCheck={false}
            disabled={!custom}
            aria-invalid={!valid}
            value={cron}
            onChange={(e) => set("cron", e.target.value)}
          />
          <p className="min-h-5 text-sm">
            {valid
              ? `${describe(cron)}. Next ${formatNext(next.at)}.`
              : next.error}
          </p>
        </Field>

        {schedule.job.kind === "task-card" && (
          <Field>
            <FieldLabel htmlFor="schedule-due">Due</FieldLabel>
            <OptionSelect
              id="schedule-due"
              value={schedule.due ?? "none"}
              options={DUES}
              align="start"
              labelClassName="w-40"
              onChange={(due) =>
                set("due", due === "none" ? undefined : due)
              }
            />
          </Field>
        )}
      </FieldGroup>

      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button disabled={!valid || !printer} onClick={() => onSave(schedule)}>
          {editing ? "Save" : "Schedule"}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
