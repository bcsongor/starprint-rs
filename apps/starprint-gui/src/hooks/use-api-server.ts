import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { startApi, stopApi, type NamedPrinter } from "@/lib/api";
import { toPrinter, type Profile } from "@/lib/settings";

/**
 * Runs the HTTP API inside the app while `enabled`, on the profiles
 * that have a host, named as the picker shows them. A profile edit
 * starts it again on the new set. Starts and stops are queued so one
 * cannot overtake the other. When the server cannot start, usually
 * because the port is taken, the error is shown and `onFail` runs so
 * the button pops back out. Returns the URL while running.
 */
export function useApiServer(
  enabled: boolean,
  profiles: Profile[],
  onFail: () => void,
): string | null {
  const [url, setUrl] = useState<string | null>(null);
  const printers: NamedPrinter[] = profiles
    .filter((p) => p.host.trim().length > 0)
    .map((p) => ({ name: p.name, printer: toPrinter(p) }));
  // Restart on a change to what is served, not on every render.
  const key = JSON.stringify(printers);
  const served = useRef(printers);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const fail = useRef(onFail);
  useEffect(() => {
    served.current = printers;
    fail.current = onFail;
  });

  useEffect(() => {
    queue.current = queue.current.then(async () => {
      try {
        if (enabled) {
          setUrl(await startApi(served.current));
        } else {
          await stopApi();
          setUrl(null);
        }
      } catch (error) {
        toast.error(
          enabled ? "The API could not start" : "The API could not stop",
          { description: String(error) },
        );
        if (enabled) {
          setUrl(null);
          fail.current();
        }
      }
    });
  }, [enabled, key]);

  return url;
}
