import { useEffect, useEffectEvent, useState } from "react";
import { toast } from "sonner";
import { Workspace } from "@/components/workspace";
import { Button } from "@/components/ui/button";
import type { Server } from "@/lib/api";
import { DEFAULT_LISTEN, LOOPBACK, startServer } from "@/lib/embedded";
import { message } from "@/lib/message";
import {
  loadSettings,
  saveSettings,
  usesRemote,
  type Settings,
} from "@/lib/settings";

/**
 * Starts the server inside the app and hands the workspace a client of
 * it, or of the other machine's server the settings name; the embedded
 * one runs either way, but holds its schedules while the other is in
 * use, since that one may keep copies of them. A change of address, or
 * of which server is in use, restarts the server; a LAN
 * address that will not bind falls back to loopback, and a start that
 * fails on loopback is shown in place of the app until a retry, or
 * going back to the default address, succeeds.
 */
export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [embedded, setEmbedded] = useState<Server | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    loadSettings().then(setSettings).catch(console.error);
  }, []);

  const update = (changes: Partial<Settings>) => {
    setSettings((current) => current && { ...current, ...changes });
    saveSettings(changes).catch(console.error);
  };

  const lanFailed = useEffectEvent((error: unknown) => {
    toast.error("The API could not listen on the LAN", {
      description: message(error),
    });
    update({ lan: false });
  });

  const lan = settings?.lan ?? false;
  const listen = settings?.listen;
  const remote = settings !== null && usesRemote(settings);
  useEffect(() => {
    if (!listen) return;
    let cancelled = false;
    const start = (ip: string) =>
      startServer({ ip, port: listen.port }, !remote);
    const started = lan
      ? start(listen.ip).catch(async (error: unknown) => {
          if (cancelled) throw error;
          // Loopback at the same port tells the causes apart. If it
          // starts, the LAN address was at fault. If it fails too, a
          // taken port or a data directory in use, the switch is left
          // alone and that failure is the one shown.
          const fallback = await start(LOOPBACK);
          if (!cancelled) lanFailed(error);
          return fallback;
        })
      : start(LOOPBACK);
    started
      .then((server) => {
        if (cancelled) return;
        setEmbedded(server);
        setFailure(null);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setEmbedded(null);
        setFailure(message(error));
      });
    return () => {
      cancelled = true;
    };
  }, [lan, listen, remote, attempt]);

  if (failure) {
    // Retrying an address that cannot be bound fails for ever, and the
    // field that changes it is in the app this screen stands in for.
    const moved =
      settings !== null &&
      (settings.lan || settings.listen.port !== DEFAULT_LISTEN.port);
    return (
      <main className="flex h-screen flex-col items-center justify-center gap-3 p-8 text-center">
        <h1 className="text-lg font-semibold">Starprint could not start</h1>
        <p className="max-w-md text-sm text-muted-foreground">{failure}</p>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setAttempt((n) => n + 1)}>
            Retry
          </Button>
          {moved && (
            <Button
              variant="outline"
              title="Listen at the default address again"
              onClick={() => update({ lan: false, listen: DEFAULT_LISTEN })}
            >
              Reset address
            </Button>
          )}
        </div>
      </main>
    );
  }
  if (!settings || !embedded) return null;

  return (
    <Workspace
      server={remote ? settings.remote : embedded}
      embedded={embedded}
      settings={settings}
      onSettings={update}
    />
  );
}
