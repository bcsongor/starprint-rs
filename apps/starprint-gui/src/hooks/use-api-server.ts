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
 * Runs the API for profiles with a host, using the stored token.
 * Profile and address changes queue a restart. A failed start shows
 * the error and calls `onFail` to turn the toggle off.
 */
export function useApiServer(
  enabled: boolean,
  profiles: Profile[],
  { ip, port }: Listen,
  onFail: () => void,
): ApiServer | null {
  const [server, setServer] = useState<ApiServer | null>(null);
  const printers: NamedPrinter[] = profiles
    .filter((p) => p.host.trim().length > 0)
    .map((p) => ({ name: p.name, printer: toPrinter(p) }));
  // Compare printer settings by value across renders.
  const key = JSON.stringify(printers);
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
          const url = await startApi(served.current, token, { ip, port });
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
  }, [enabled, key, ip, port]);

  return server;
}
