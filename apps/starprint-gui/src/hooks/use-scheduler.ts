import { useEffect, useEffectEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  SCHEDULE_RAN,
  setSchedules,
  type ScheduleOutcome,
  type Scheduled,
} from "@/lib/api";
import { summary } from "@/lib/schedule";
import { toPrinter, type Profile, type Schedules } from "@/lib/settings";

/**
 * Syncs enabled schedules and reports runs as toasts. Waits for the store
 * to load: an empty first sync would wipe the run record. A refused list
 * turns schedules off through `onFail`.
 */
export function useScheduler(
  schedules: Schedules | null,
  profiles: Profile[],
  onFail: () => void,
) {
  const scheduled: Scheduled[] = schedules?.running
    ? schedules.items.flatMap((s) => {
        const profile = profiles.find((p) => p.id === s.profileId);
        if (!s.enabled || !profile) return [];
        const { id, job, cron, due } = s;
        return [{ id, job, printer: toPrinter(profile), cron, due }];
      })
    : [];
  // Compare by value across renders; null until the store has loaded.
  const key = schedules && JSON.stringify(scheduled);
  const fail = useEffectEvent(onFail);
  useEffect(() => {
    if (key === null) return;
    setSchedules(JSON.parse(key)).catch((error) => {
      toast.error("Schedules could not start", { description: String(error) });
      fail();
    });
  }, [key]);

  const report = useEffectEvent(({ id, error }: ScheduleOutcome) => {
    const item = schedules?.items.find((s) => s.id === id);
    const what = item ? summary(item.job) : "a scheduled job";
    if (error) toast.error(`Could not print ${what}`, { description: error });
    else toast.success(`Printed ${what} on schedule`);
  });
  useEffect(() => {
    const unlisten = listen<ScheduleOutcome>(SCHEDULE_RAN, (event) =>
      report(event.payload),
    );
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);
}
