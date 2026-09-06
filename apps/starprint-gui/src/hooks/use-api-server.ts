import { useEffect, useEffectEvent, useRef, useState } from "react";
import { toast } from "sonner";
import { startApi, stopApi, type Listen, type NamedPrinter } from "@/lib/api";
import { loadApiToken, toPrinter, type Profile } from "@/lib/settings";

/** The running API, for the toolbar to show and for a client to copy. */
export interface ApiServer {
  url: string;
  token: string;
}

/**
 * Runs the HTTP API inside the app while `enabled`, on the profiles
 * that have a host, named as the picker shows them, at `listen`, behind
 * the stored token. A profile edit or a new address starts it again.
 * Starts and stops are queued so one cannot overtake the other. When
 * the server cannot start, because the port is taken or the address is
 * no longer this machine's, the error is shown and `onFail` runs so
 * the button pops back out. Returns the URL and token while running,
 * for the button to show.
 */
export function useApiServer(
  enabled: boolean,
  profiles: Profile[],
  listen: Listen,
  onFail: () => void,
): ApiServer | null {
  const [server, setServer] = useState<ApiServer | null>(null);
  const printers: NamedPrinter[] = profiles
    .filter((p) => p.host.trim().length > 0)
    .map((p) => ({ name: p.name, printer: toPrinter(p) }));
  // Restart on a change to what is served or where, not on every render.
  const key = JSON.stringify([listen, printers]);
  const served = useRef(printers);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const fail = useEffectEvent(onFail);
  useEffect(() => {
    served.current = printers;
  });

  useEffect(() => {
    queue.current = queue.current.then(async () => {
      try {
        if (enabled) {
          const token = await loadApiToken();
          const url = await startApi(served.current, token, listen);
          setServer({ url, token });
        } else {
          await stopApi();
          setServer(null);
        }
      } catch (error) {
        toast.error(
          enabled ? "The API could not start" : "The API could not stop",
          { description: String(error) },
        );
        if (enabled) {
          setServer(null);
          fail();
        }
      }
    });
  }, [enabled, key, listen]);

  return server;
}
