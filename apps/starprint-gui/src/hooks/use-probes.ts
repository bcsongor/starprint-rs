import { useEffect, useRef, useState } from "react";
import { probePrinter } from "@/lib/api";
import type { Profile } from "@/lib/settings";

const INTERVAL_MS = 10_000;
/** Lets rapid profile changes settle into one round. */
const DEBOUNCE_MS = 150;
const RETRY_MS = 150;

export type Status = "checking" | "online" | "offline";

const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

const endpoint = ({ host, port }: Pick<Profile, "host" | "port">) =>
  `${host.trim()}:${port}`;

/**
 * Status per endpoint; the active profile is probed first. A failed
 * probe is retried once, and probing pauses while a job prints.
 */
export function useProbes(
  profiles: Profile[],
  activeId: string,
  paused: boolean,
): (profile: Profile) => Status {
  const [statuses, setStatuses] = useState<Record<string, Status>>({});
  // Re-probe when a host or port changes, not on every profile edit.
  const targets = [...profiles]
    .sort((a, b) => Number(b.id === activeId) - Number(a.id === activeId))
    .map((p) => ({ host: p.host, port: p.port }));
  const key = JSON.stringify(targets);
  const latest = useRef(targets);
  useEffect(() => {
    latest.current = targets;
  });

  useEffect(() => {
    if (paused) return;

    // One probe per endpoint; two at once would refuse each other.
    const endpoints = new Map(
      latest.current.map((printer) => [endpoint(printer), printer]),
    );

    let cancelled = false;
    const probe = (host: string, port: number) =>
      probePrinter(host, port).catch(() => false);

    const round = () =>
      Promise.all(
        [...endpoints].map(async ([address, { host, port }]) => {
          let up = await probe(host, port);
          if (!up && !cancelled && host.trim()) {
            await sleep(RETRY_MS);
            if (cancelled) return;
            up = await probe(host, port);
          }
          if (cancelled) return;
          const status = up ? "online" : "offline";
          setStatuses((prev) => ({ ...prev, [address]: status }));
        }),
      );

    const first = setTimeout(round, DEBOUNCE_MS);
    const timer = setInterval(round, INTERVAL_MS);
    return () => {
      cancelled = true;
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [key, paused]);

  return (profile) => statuses[endpoint(profile)] ?? "checking";
}

export function statusLabel(profile: Profile, status: Status): string {
  if (!profile.host.trim()) return "No host set";
  switch (status) {
    case "online":
      return `${profile.host} is reachable`;
    case "offline":
      return `${profile.host} is not answering`;
    default:
      return `Checking ${profile.host}…`;
  }
}
