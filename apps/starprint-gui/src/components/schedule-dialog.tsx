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
import { useAsync } from "@/hooks/use-async";
import { nextRun, type Due } from "@/lib/api";
import {
  NAMES,
  PRESETS,
  describe,
  formatNext,
  fromCron,
  summary,
  toCron,
  type Preset,
} from "@/lib/schedule";
import type { Profile, Schedule } from "@/lib/settings";

/** What the dialog opens on: a schedule, and whether it exists yet. */
export interface Draft {
  schedule: Schedule;
  editing: boolean;
}

interface Props {
  draft: Draft | null;
  profiles: Profile[];
  onClose: () => void;
  onSave: (schedule: Schedule) => void;
}

/** Sets a schedule's printer and timetable. The job is fixed. */
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
          draft={draft}
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

/** The next run of an expression, or the parser's complaint about it. */
type Next = { cron: string; at: string } | { cron: string; error: string };

function ScheduleForm({
  draft: { schedule: initial, editing },
  profiles,
  onCancel,
  onSave,
}: {
  draft: Draft;
  profiles: Profile[];
  onCancel: () => void;
  onSave: (schedule: Schedule) => void;
}) {
  const [schedule, setSchedule] = useState(initial);
  const spelled = fromCron(initial.cron);
  const [preset, setPreset] = useState<Preset | "custom">(
    spelled?.preset ?? "custom",
  );
  const [time, setTime] = useState(spelled?.time ?? "09:00");
  const set = <K extends keyof Schedule>(key: K, value: Schedule[K]) =>
    setSchedule({ ...schedule, [key]: value });

  const { cron } = schedule;
  const checked = useAsync<Next>(
    () =>
      nextRun(cron)
        .then((at) => ({ cron, at }))
        .catch((error) => ({ cron, error: String(error) })),
    [cron],
  );
  // The last answer stays until the next lands, and is for an older expression.
  const next = checked?.cron === cron ? checked : null;
  const valid = next !== null && "at" in next;
  const printer = profiles.find((p) => p.id === schedule.profileId);
  const hasHost = Boolean(printer?.host.trim());

  return (
    <DialogContent className="gap-4 sm:max-w-md">
      <DialogHeader>
        <DialogTitle>
          {editing ? "Edit schedule" : `Schedule ${NAMES[schedule.job.kind]}`}
        </DialogTitle>
        <DialogDescription>
          {editing
            ? "Prints as it was scheduled. Load it into its form to change the job."
            : "Prints it as set up now, at these times, while schedules run."}
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
            value={schedule.profileId}
            onValueChange={(id) => {
              if (id) set("profileId", id);
            }}
          >
            <SelectTrigger id="schedule-printer" className="w-full">
              <SelectValue>{printer?.name}</SelectValue>
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false} align="start">
              {profiles.map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                  <span className="text-muted-foreground">
                    {" "}
                    · {p.host || "no host"}
                  </span>
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
                value={preset}
                options={WHEN}
                align="start"
                labelClassName="w-52"
                onChange={(next) => {
                  setPreset(next);
                  if (next !== "custom") set("cron", toCron(next, time));
                }}
              />
            </div>
            <Input
              type="time"
              aria-label="Time"
              className="w-28 shrink-0 tabular-nums"
              value={time}
              disabled={preset === "custom"}
              onChange={(e) => {
                setTime(e.target.value);
                if (preset !== "custom") {
                  set("cron", toCron(preset, e.target.value));
                }
              }}
            />
          </div>
        </Field>

        <Field data-invalid={next !== null && !valid}>
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
            aria-invalid={next !== null && !valid}
            value={schedule.cron}
            onChange={(e) => {
              set("cron", e.target.value);
              setPreset(fromCron(e.target.value)?.preset ?? "custom");
            }}
          />
          <p className="min-h-5 text-sm">
            {next === null
              ? null
              : "at" in next
                ? `${describe(schedule.cron)}. Next ${formatNext(next.at)}.`
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
              onChange={(due) => set("due", due === "none" ? null : due)}
            />
          </Field>
        )}
      </FieldGroup>

      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          disabled={!valid || !hasHost}
          onClick={() => onSave(schedule)}
        >
          {editing ? "Save" : "Schedule"}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
