import { useEffect, useEffectEvent, useState } from "react";
import { toast } from "sonner";
import { Workspace } from "@/components/workspace";
import { Button } from "@/components/ui/button";
import type { Server } from "@/lib/api";
import { LOOPBACK, startServer } from "@/lib/embedded";
import { loadSettings, saveSettings, type Settings } from "@/lib/settings";

/**
 * Starts the server inside the app and hands the workspace a client of
 * it, or of the other machine's server the settings name; the embedded
 * one runs either way. A change of address restarts the server; a LAN
 * address that will not bind falls back to loopback, and a start that
 * fails on loopback is shown in place of the app until a retry succeeds.
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

  const fail = useEffectEvent((error: unknown, lan: boolean) => {
    if (lan) {
      toast.error("The API could not listen on the LAN", {
        description: String(error),
      });
      update({ lan: false });
    } else {
      setEmbedded(null);
      setFailure(String(error));
    }
  });

  const lan = settings?.lan ?? false;
  const listen = settings?.listen;
  useEffect(() => {
    if (!listen) return;
    let cancelled = false;
    startServer({ ip: lan ? listen.ip : LOOPBACK, port: listen.port })
      .then((started) => {
        if (cancelled) return;
        setEmbedded(started);
        setFailure(null);
      })
      .catch((error) => {
        if (!cancelled) fail(error, lan);
      });
    return () => {
      cancelled = true;
    };
  }, [lan, listen, attempt]);

  if (failure) {
    return (
      <main className="flex h-screen flex-col items-center justify-center gap-3 p-8 text-center">
        <h1 className="text-lg font-semibold">Starprint could not start</h1>
        <p className="max-w-md text-sm text-muted-foreground">{failure}</p>
        <Button variant="outline" onClick={() => setAttempt((n) => n + 1)}>
          Retry
        </Button>
      </main>
    );
  }
  if (!settings || !embedded) return null;

  return (
    <Workspace
      server={settings.useRemote ? settings.remote : embedded}
      embedded={embedded}
      settings={settings}
      onSettings={update}
    />
  );
}
