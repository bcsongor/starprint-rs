import { useCallback, useEffect, useState } from "react";
import type { Client, Profile, Schedule } from "@/lib/api";

const POLL_MS = 10_000;

export type Status = "checking" | "online" | "offline";

/**
 * The server's profiles and schedules, and whether each printer
 * answers, read every ten seconds so a change made over the API by
 * another program shows up. `refresh` reads them now, after a change
 * made here. The server takes the printer's turn for a status, so a
 * probe never lands in the middle of a job.
 */
export function useServer(api: Client) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [schedules, setSchedules] = useState<Schedule[] | null>(null);
  const [statuses, setStatuses] = useState<Record<string, Status>>({});

  const refresh = useCallback(async () => {
    const [profiles, schedules] = await Promise.all([
      api.printers(),
      api.schedules(),
    ]);
    setProfiles(profiles);
    setSchedules(schedules);
    for (const { name } of profiles) {
      api
        .status(name)
        .then((online) =>
          setStatuses((prev) => ({
            ...prev,
            [name]: online ? "online" : "offline",
          })),
        )
        .catch(console.error);
    }
  }, [api]);

  useEffect(() => {
    const tick = () => refresh().catch(console.error);
    tick();
    const timer = setInterval(tick, POLL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  return {
    profiles,
    schedules,
    status: (name: string): Status => statuses[name] ?? "checking",
    refresh,
  };
}

export function statusLabel(profile: Profile, status: Status): string {
  switch (status) {
    case "online":
      return `${profile.host} is reachable`;
    case "offline":
      return `${profile.host} is not answering`;
    default:
      return `Checking ${profile.host}…`;
  }
}
