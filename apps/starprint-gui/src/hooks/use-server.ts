import { useCallback, useEffect, useRef, useState } from "react";
import {
  byName,
  type Client,
  type FaxLine,
  type Profile,
  type Schedule,
} from "@/lib/api";

const POLL_MS = 10_000;

export type Status = "checking" | "online" | "offline";

/** What a server has answered so far. */
interface Read {
  /** Whether the server answered the last read. */
  reachable: Status;
  /** The server's version, once it has answered. */
  version: string | null;
  /** In the order the pickers list them. */
  profiles: Profile[];
  schedules: Schedule[] | null;
  /** Null for a server that has no fax line to read, too. */
  fax: FaxLine | null;
  statuses: Record<string, Status>;
}

const UNREAD: Read = {
  reachable: "checking",
  version: null,
  profiles: [],
  schedules: null,
  fax: null,
  statuses: {},
};

/**
 * The server's profiles, schedules and fax line, and whether each
 * printer answers, read every ten seconds so a change made over the API
 * by another program shows up. `refresh` reads them now, after a change
 * made here. The server takes the printer's turn for a status, so a
 * probe never lands in the middle of a job.
 */
export function useServer(api: Client) {
  const [read, setRead] = useState({ api, ...UNREAD });
  // Another server's answers are not this one's: start over on a switch.
  if (read.api !== api) setRead({ api, ...UNREAD });
  // Counts the reads begun. One that a later read has overtaken is
  // dropped, so a poll still out when a change was made here cannot put
  // the old state back.
  const begun = useRef(0);

  const refresh = useCallback(async () => {
    const mine = ++begun.current;
    /** Applies an answer unless the client has since been replaced. */
    const answer = (changes: (read: Read) => Partial<Read>) =>
      setRead((prev) =>
        prev.api === api ? { ...prev, ...changes(prev) } : prev,
      );
    try {
      const [{ version, printers }, schedules, fax] = await Promise.all([
        api.printers(),
        api.schedules(),
        // A server from before fax still has printers to show.
        api.fax().catch(() => null),
      ]);
      if (mine !== begun.current) return;
      answer(() => ({
        reachable: "online",
        version,
        profiles: byName(printers),
        schedules,
        fax,
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
      if (mine !== begun.current) return;
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
    fax: read.fax,
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
