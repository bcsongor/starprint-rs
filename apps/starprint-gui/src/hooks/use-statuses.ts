import { useEffect, useState } from "react";
import type { Client, Profile } from "@/lib/api";

const INTERVAL_MS = 10_000;

export type Status = "checking" | "online" | "offline";

/**
 * Whether each printer answers, asked of the server every ten seconds.
 * The server takes the printer's turn for it, so a probe never lands in
 * the middle of a job.
 */
export function useStatuses(
  api: Client,
  profiles: Profile[],
): (name: string) => Status {
  const [statuses, setStatuses] = useState<Record<string, Status>>({});
  useEffect(() => {
    let cancelled = false;
    const round = () => {
      for (const { name } of profiles) {
        api
          .status(name)
          .then((online) => {
            if (cancelled) return;
            setStatuses((prev) => ({
              ...prev,
              [name]: online ? "online" : "offline",
            }));
          })
          .catch(console.error);
      }
    };
    round();
    const timer = setInterval(round, INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [api, profiles]);
  return (name) => statuses[name] ?? "checking";
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
