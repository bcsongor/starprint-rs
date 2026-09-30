import { useCallback, useEffect, useState } from "react";
import type { Client, FaxLine } from "@/lib/api";

const POLL_MS = 10_000;

/**
 * The server's fax line, read every ten seconds. `refresh` reads it now,
 * after a change made here. Null until the server has answered.
 */
export function useFax(api: Client) {
  const [read, setRead] = useState<{ api: Client; fax: FaxLine | null }>({
    api,
    fax: null,
  });
  // Another server's line is not this one's: start over on a switch.
  if (read.api !== api) setRead({ api, fax: null });

  const refresh = useCallback(async () => {
    const fax = await api.fax();
    setRead((prev) => (prev.api === api ? { api, fax } : prev));
  }, [api]);

  useEffect(() => {
    const tick = () => refresh().catch(console.error);
    tick();
    const timer = setInterval(tick, POLL_MS);
    return () => clearInterval(timer);
  }, [refresh]);

  return { fax: read.api === api ? read.fax : null, refresh };
}
