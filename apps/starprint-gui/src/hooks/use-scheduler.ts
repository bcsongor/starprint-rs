import { useEffect, useEffectEvent, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { SCHEDULE_RAN, setSchedules, type ScheduleOutcome } from "@/lib/api";
import { summary, toScheduled } from "@/lib/schedule";
import type { Profile, Schedule } from "@/lib/settings";

/** Hands the enabled schedules to the Rust side and reports their runs. */
export function useScheduler(
  schedules: Schedule[] | null,
  profiles: Profile[],
) {
  const scheduled = useMemo(
    () =>
      schedules
        ?.filter((s) => s.enabled)
        .flatMap((s) => toScheduled(s, profiles) ?? []),
    [schedules, profiles],
  );
  useEffect(() => {
    if (scheduled) setSchedules(scheduled).catch(console.error);
  }, [scheduled]);

  const report = useEffectEvent(({ id, error }: ScheduleOutcome) => {
    const item = schedules?.find((s) => s.id === id);
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
