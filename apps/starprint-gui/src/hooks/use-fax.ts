import { useCallback, useEffect, useState } from "react";
import type { Client, FaxLine } from "@/lib/api";

const POLL_MS = 10_000;
/** While a number is being mined, so the progress bar moves. */
const ACTIVATING_MS = 1_000;

/**
 * The server's fax line, read every ten seconds, or every second while
 * it is being activated. `refresh` reads it now, after a change made
 * here. Null until the server has answered.
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

  const activating = read.fax?.activation != null;
  useEffect(() => {
    const tick = () => refresh().catch(console.error);
    tick();
    const timer = setInterval(tick, activating ? ACTIVATING_MS : POLL_MS);
    return () => clearInterval(timer);
  }, [refresh, activating]);

  return { fax: read.api === api ? read.fax : null, refresh };
}
