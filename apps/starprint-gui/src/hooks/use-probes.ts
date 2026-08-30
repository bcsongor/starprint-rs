import { useEffect, useState } from "react";
import { probePrinter } from "@/lib/api";
import type { Profile } from "@/lib/settings";

const INTERVAL_MS = 10_000;
/** Lets rapid profile changes settle into one round. */
const DEBOUNCE_MS = 150;
const RETRY_MS = 150;

export type Status = "checking" | "online" | "offline";

const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));

/**
 * Status per profile id; the active profile is probed first. A failed
 * probe is retried once, and probing pauses while a job prints.
 */
export function useProbes(
  profiles: Profile[],
  activeId: string,
  paused: boolean,
): Record<string, Status> {
  const [statuses, setStatuses] = useState<Record<string, Status>>({});
  // Re-probe when a host or port changes, not on every profile edit.
  const key = [...profiles]
    .sort((a, b) => Number(b.id === activeId) - Number(a.id === activeId))
    .map((p) => `${p.id}\t${p.host}\t${p.port}`)
    .join("\n");

  useEffect(() => {
    if (paused) return;

    const targets = key
      .split("\n")
      .filter(Boolean)
      .map((line) => {
        const [id, host, port] = line.split("\t");
        return { id, host, port: Number(port) };
      });

    // One probe per endpoint; two at once would refuse each other.
    const endpoints = new Map<
      string,
      { host: string; port: number; ids: string[] }
    >();
    for (const { id, host, port } of targets) {
      const endpoint = `${host}:${port}`;
      const existing = endpoints.get(endpoint);
      if (existing) existing.ids.push(id);
      else endpoints.set(endpoint, { host, port, ids: [id] });
    }

    let cancelled = false;
    const probe = (host: string, port: number) =>
      probePrinter(host, port).catch(() => false);

    const round = () =>
      Promise.all(
        [...endpoints.values()].map(async ({ host, port, ids }) => {
          let up = await probe(host, port);
          if (!up && !cancelled && host.trim()) {
            await sleep(RETRY_MS);
            if (cancelled) return;
            up = await probe(host, port);
          }
          if (cancelled) return;
          const status = up ? "online" : "offline";
          setStatuses((prev) => {
            const next = { ...prev };
            for (const id of ids) next[id] = status;
            return next;
          });
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

  return statuses;
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
