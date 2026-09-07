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
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Switch } from "@/components/ui/switch";
import type { Job } from "@/lib/api";
import { describe, summary } from "@/lib/schedule";
import type { Profile, Schedule } from "@/lib/settings";
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
  schedules: Schedule[];
  onChange: (schedules: Schedule[]) => void;
  profiles: Profile[];
  onEdit: (schedule: Schedule) => void;
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
  onLoad,
}: Props) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="gap-0 sm:max-w-md">
        <SheetHeader className="border-b">
          <SheetTitle>Schedules</SheetTitle>
          <SheetDescription>
            Jobs printed on a timetable while the app is open.
          </SheetDescription>
        </SheetHeader>
        <ul className="flex flex-col overflow-y-auto p-2">
          {schedules.map((s) => {
            const Icon = ICONS[s.job.kind];
            const profile = profiles.find((p) => p.id === s.profileId);
            const hasHost = Boolean(profile?.host.trim());
            return (
              <li
                key={s.id}
                className="flex items-center gap-3 rounded-md px-2 py-2"
              >
                <Switch
                  checked={s.enabled}
                  aria-label="Enabled"
                  onCheckedChange={(enabled) =>
                    onChange(
                      schedules.map((item) =>
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
                  <div className="shrink-0 text-xs text-muted-foreground">
                    {!s.enabled ? "Off" : hasHost ? null : "No host"}
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
                    <DropdownMenuItem onClick={() => onLoad(s)}>
                      Load into form
                    </DropdownMenuItem>
                    <DropdownMenuSeparator />
                    <DropdownMenuItem
                      variant="destructive"
                      onClick={() =>
                        onChange(schedules.filter((item) => item.id !== s.id))
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
