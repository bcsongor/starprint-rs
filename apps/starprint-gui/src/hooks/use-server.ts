import { useCallback, useEffect, useState } from "react";
import type { Client, Profile, Schedule } from "@/lib/api";

const POLL_MS = 10_000;

export type Status = "checking" | "online" | "offline";

/** What a server has answered so far. */
interface Read {
  /** Whether the server answered the last read. */
  reachable: Status;
  /** The server's version, once it has answered. */
  version: string | null;
  profiles: Profile[];
  schedules: Schedule[] | null;
  statuses: Record<string, Status>;
}

const UNREAD: Read = {
  reachable: "checking",
  version: null,
  profiles: [],
  schedules: null,
  statuses: {},
};

/**
 * The server's profiles and schedules, and whether each printer
 * answers, read every ten seconds so a change made over the API by
 * another program shows up. `refresh` reads them now, after a change
 * made here. The server takes the printer's turn for a status, so a
 * probe never lands in the middle of a job.
 */
export function useServer(api: Client) {
  const [read, setRead] = useState({ api, ...UNREAD });
  // Another server's answers are not this one's: start over on a switch.
  if (read.api !== api) setRead({ api, ...UNREAD });

  const refresh = useCallback(async () => {
    /** Applies an answer unless the client has since been replaced. */
    const answer = (changes: (read: Read) => Partial<Read>) =>
      setRead((prev) =>
        prev.api === api ? { ...prev, ...changes(prev) } : prev,
      );
    try {
      const [{ version, printers }, schedules] = await Promise.all([
        api.printers(),
        api.schedules(),
      ]);
      answer(() => ({
        reachable: "online",
        version,
        profiles: printers,
        schedules,
      }));
      for (const { name } of printers) {
        api
          .status(name)
          .then((online) =>
            answer(({ statuses }) => ({
              statuses: { ...statuses, [name]: online ? "online" : "offline" },
            })),
          )
          .catch(console.error);
      }
    } catch (error) {
      // A server that is not answering has nothing to show.
      answer(() => ({ ...UNREAD, reachable: "offline" }));
      throw error;
    }
  }, [api]);

  useEffect(() => {
    const tick = () => refresh().catch(console.error);
    tick();
    const timer = setInterval(tick, POLL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  return {
    reachable: read.reachable,
    version: read.version,
    profiles: read.profiles,
    schedules: read.schedules,
    status: (name: string): Status => read.statuses[name] ?? "checking",
    refresh,
  };
}

/** What the dot beside `host`, a printer's or a server's, says. */
export function statusLabel(host: string, status: Status): string {
  switch (status) {
    case "online":
      return `${host} is reachable`;
    case "offline":
      return `${host} is not answering`;
    default:
      return `Checking ${host}…`;
  }
}
