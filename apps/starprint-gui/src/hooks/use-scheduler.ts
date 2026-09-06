import { useEffect, useEffectEvent, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { SCHEDULE_RAN, setSchedules, type ScheduleOutcome } from "@/lib/api";
import { summary, toScheduled } from "@/lib/schedule";
import type { Profile, Schedules } from "@/lib/settings";

/**
 * Waits for settings and the run listener so startup failures can be
 * reported. An empty first sync would wipe the run record. A refused
 * schedule list turns schedules off through `onFail`.
 */
export function useScheduler(
  schedules: Schedules | null,
  profiles: Profile[],
  onFail: () => void,
) {
  const [listening, setListening] = useState(false);
  const scheduled = schedules?.running
    ? schedules.items
        .filter((s) => s.enabled)
        .flatMap((s) => toScheduled(s, profiles) ?? [])
    : [];
  // Compare by value across renders; null until the store has loaded.
  const key = schedules && JSON.stringify(scheduled);
  const fail = useEffectEvent(onFail);
  useEffect(() => {
    if (key === null || !listening) return;
    setSchedules(JSON.parse(key)).catch((error) => {
      toast.error("Schedules could not start", { description: String(error) });
      fail();
    });
  }, [key, listening]);

  const report = useEffectEvent(({ id, error }: ScheduleOutcome) => {
    const item = schedules?.items.find((s) => s.id === id);
    const what = item ? summary(item.job) : "a scheduled job";
    if (error) toast.error(`Could not print ${what}`, { description: error });
    else toast.success(`Printed ${what} on schedule`);
  });
  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<ScheduleOutcome>(SCHEDULE_RAN, (event) =>
      report(event.payload),
    );
    unlisten.then(() => {
      if (!cancelled) setListening(true);
    });
    return () => {
      cancelled = true;
      unlisten.then((stop) => stop());
    };
  }, []);
}
