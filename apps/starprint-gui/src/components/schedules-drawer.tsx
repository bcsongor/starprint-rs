import { useEffect, useState } from "react";
import {
  EllipsisIcon,
  FlaskConicalIcon,
  ImageIcon,
  QrCodeIcon,
  SquareCheckIcon,
  StickyNoteIcon,
  TypeIcon,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { FieldLabel } from "@/components/ui/field";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Switch } from "@/components/ui/switch";
import { useAsync } from "@/hooks/use-async";
import { nextRun, type Job } from "@/lib/api";
import { describe, formatSoon, summary } from "@/lib/schedule";
import type { Profile, Schedule, Schedules } from "@/lib/settings";
import { cn } from "@/lib/utils";

const ICONS: Record<Job["kind"], LucideIcon> = {
  "task-card": SquareCheckIcon,
  text: TypeIcon,
  note: StickyNoteIcon,
  qr: QrCodeIcon,
  picture: ImageIcon,
  "test-page": FlaskConicalIcon,
};

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  schedules: Schedules;
  onChange: (schedules: Schedules) => void;
  profiles: Profile[];
  onEdit: (schedule: Schedule) => void;
  onPrint: (schedule: Schedule) => void;
  /** Puts the job back in its form, to change and schedule again. */
  onLoad: (schedule: Schedule) => void;
}

export function SchedulesDrawer({
  open,
  onOpenChange,
  schedules,
  onChange,
  profiles,
  onEdit,
  onPrint,
  onLoad,
}: Props) {
  const { running, items } = schedules;
  const setItems = (items: Schedule[]) => onChange({ ...schedules, items });

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="gap-0 sm:max-w-md">
        <SheetHeader className="border-b">
          <SheetTitle>Schedules</SheetTitle>
          <SheetDescription>
            Jobs printed on a timetable while the app is open.
          </SheetDescription>
        </SheetHeader>
        <div className="flex h-12 items-center gap-2 border-b bg-muted px-4">
          <Switch
            id="run-schedules"
            checked={running}
            onCheckedChange={(running) => onChange({ ...schedules, running })}
          />
          <FieldLabel
            htmlFor="run-schedules"
            className="text-sm tracking-normal normal-case text-foreground"
          >
            Run schedules
          </FieldLabel>
        </div>
        <ul
          className={cn(
            "flex flex-col overflow-y-auto p-2",
            !running && "opacity-50",
          )}
        >
          {items.map((s) => {
            const Icon = ICONS[s.job.kind];
            const profile = profiles.find((p) => p.id === s.profileId);
            return (
              <li
                key={s.id}
                className="flex items-center gap-3 rounded-md px-2 py-2"
              >
                <Switch
                  checked={s.enabled}
                  aria-label="Enabled"
                  onCheckedChange={(enabled) =>
                    setItems(
                      items.map((item) =>
                        item.id === s.id ? { ...item, enabled } : item,
                      ),
                    )
                  }
                />
                <div
                  className={cn(
                    "flex min-w-0 flex-1 items-center gap-3",
                    !s.enabled && "opacity-50",
                  )}
                >
                  <Icon
                    className="size-4 shrink-0 text-muted-foreground"
                    aria-hidden
                  />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm">{summary(s.job)}</div>
                    <div className="truncate text-xs text-muted-foreground">
                      {describe(s.cron)} · {profile?.name ?? "no printer"}
                    </div>
                  </div>
                  <div className="shrink-0 text-xs text-muted-foreground tabular-nums">
                    {s.enabled ? <NextRun key={s.cron} cron={s.cron} /> : "Off"}
                  </div>
                </div>
                <DropdownMenu modal={false}>
                  <DropdownMenuTrigger
                    render={
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label="Schedule actions"
                      />
                    }
                  >
                    <EllipsisIcon />
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem onClick={() => onEdit(s)}>
                      Edit…
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      disabled={!profile}
                      onClick={() => onPrint(s)}
                    >
                      Print now
                    </DropdownMenuItem>
                    <DropdownMenuItem onClick={() => onLoad(s)}>
                      Load into form
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      variant="destructive"
                      onClick={() =>
                        setItems(items.filter((item) => item.id !== s.id))
                      }
                    >
                      Delete
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </li>
            );
          })}
        </ul>
        <p className="mt-auto border-t p-4 text-sm text-muted-foreground">
          Set a job up under its tab and press Schedule to add it here.
        </p>
      </SheetContent>
    </Sheet>
  );
}

function NextRun({ cron }: { cron: string }) {
  // Once a minute, so a row left open across its run moves on.
  const [minute, setMinute] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => setMinute((n) => n + 1), 60_000);
    return () => clearInterval(timer);
  }, []);
  const next = useAsync(() => nextRun(cron).catch(() => null), [cron, minute]);
  return next ? formatSoon(next) : null;
}
