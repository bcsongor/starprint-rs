import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { CopyIcon } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { toast } from "sonner";
import { ConnectionDot, DOTTED } from "@/components/connection-dot";
import { OptionSelect } from "@/components/option-select";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Switch } from "@/components/ui/switch";
import { statusLabel, type Status } from "@/hooks/use-server";
import type { Server } from "@/lib/api";
import { LOOPBACK, listAddresses, type Address } from "@/lib/embedded";
import type { Settings } from "@/lib/settings";

interface Props {
  /** The server the app prints through, and whether it answers. */
  server: Server;
  reachable: Status;
  /** That server's version, once it has answered. */
  version: string | null;
  /** The server this computer runs, in use or not. */
  embedded: Server;
  settings: Settings;
  onSettings: (changes: Partial<Settings>) => void;
}

/**
 * Where this computer's server is and what it wants, for programs that
 * print through it, and the choice to print through another machine's
 * server instead.
 */
export function ApiButton({
  server,
  reachable,
  version,
  embedded,
  settings: { lan, listen, remote, useRemote },
  onSettings,
}: Props) {
  const [open, setOpen] = useState(false);
  const [addresses, setAddresses] = useState<Address[]>([]);
  const [app, setApp] = useState<string | null>(null);

  useEffect(() => {
    getVersion().then(setApp).catch(console.error);
  }, []);

  // Read each time the details open, so a network joined since shows.
  useEffect(() => {
    if (!open) return;
    listAddresses().then(setAddresses).catch(console.error);
  }, [open]);

  // The stored address may belong to a network this machine has left;
  // keep it listed so the select can still show it. Loopback is the
  // fresh install's, and shows as nothing chosen.
  const options = (
    listen.ip === LOOPBACK || addresses.some((a) => a.ip === listen.ip)
      ? addresses
      : [...addresses, { ip: listen.ip, name: "not on this machine" }]
  ).map((a) => ({ value: a.ip, label: a.ip, hint: a.name }));

  /** A fresh install has no LAN address yet; the first adapter's is taken. */
  const setLan = (lan: boolean) => {
    if (lan && listen.ip === LOOPBACK) {
      const first = addresses.at(0);
      if (!first) {
        toast.error("This machine has no LAN address");
        return;
      }
      onSettings({ lan, listen: { ...listen, ip: first.ip } });
    } else {
      onSettings({ lan });
    }
  };

  const setPort = (typed: string) => {
    const port = Number(typed);
    if (Number.isInteger(port) && port >= 1 && port <= 65535) {
      onSettings({ listen: { ...listen, port } });
    }
  };

  // The switch flips before the server has restarted, so the QR waits
  // for the address the server actually bound.
  const onLan = new URL(embedded.url).hostname !== LOOPBACK;

  const older =
    app !== null &&
    version !== null &&
    version.localeCompare(app, undefined, { numeric: true }) < 0;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            variant="outline"
            title="HTTP API"
            aria-label="HTTP API"
            className={DOTTED}
          />
        }
      >
        <ConnectionDot
          status={reachable}
          label={statusLabel(server.url, reachable)}
        />
        API
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-auto min-w-md gap-0 overflow-hidden p-0 [&_input]:bg-background [&_[data-slot=select-trigger]]:bg-background [&_[data-slot=select-value]]:font-mono"
      >
        <div className="flex flex-col gap-2 border-b bg-muted p-2.5">
          <Toggle id="api-lan" checked={lan} onCheckedChange={setLan}>
            Also listen on the LAN
          </Toggle>
          <Row label="Listen">
            <OptionSelect
              id="api-address"
              value={listen.ip}
              options={options}
              disabled={!lan}
              onChange={(ip) => onSettings({ listen: { ...listen, ip } })}
              align="start"
              labelClassName="w-32 font-mono"
            />
            <CommitInput
              id="api-port"
              aria-label="Port"
              title="Port"
              inputMode="numeric"
              value={String(listen.port)}
              onCommit={setPort}
              className="w-18 shrink-0 text-center font-mono"
            />
          </Row>
        </div>
        <div className="flex flex-col gap-2 p-2.5">
          <Row label="URL">
            <ReadOnlyField value={embedded.url} />
          </Row>
          <Row label="Token">
            <ReadOnlyField value={embedded.token} />
          </Row>
          {/* The page reads the token from the fragment, which stays on
              the phone. Loopback would send the phone to itself. */}
          {onLan ? (
            <div className="flex items-center gap-3 pt-1">
              <QRCodeSVG
                value={`${embedded.url}/#token=${encodeURIComponent(embedded.token)}`}
                size={96}
                marginSize={1}
                className="rounded-sm bg-white"
              />
              <p className="text-xs text-muted-foreground">
                Scan with a phone on the same network to print from it.
              </p>
            </div>
          ) : (
            <p className="text-xs text-muted-foreground">
              Listen on the LAN to get a code a phone can scan.
            </p>
          )}
        </div>
        <div className="flex flex-col gap-2 border-t bg-muted p-2.5">
          <Toggle
            id="api-remote"
            checked={useRemote}
            onCheckedChange={(useRemote) => onSettings({ useRemote })}
          >
            Print through another server
          </Toggle>
          <Row label="URL">
            <CommitInput
              id="api-remote-url"
              placeholder="http://192.168.1.50:9110"
              value={remote.url}
              onCommit={(url) =>
                onSettings({
                  remote: { ...remote, url: url.trim().replace(/\/+$/, "") },
                })
              }
              className="font-mono"
            />
          </Row>
          <Row label="Token">
            <CommitInput
              id="api-remote-token"
              value={remote.token}
              onCommit={(token) => onSettings({ remote: { ...remote, token } })}
              className="font-mono"
            />
          </Row>
          {older && (
            <p className="text-xs text-muted-foreground">
              Server {version} is older than this app ({app}).
            </p>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

function Toggle({
  id,
  checked,
  onCheckedChange,
  children,
}: {
  id: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  children: React.ReactNode;
}) {
  return (
    <Field orientation="horizontal" className="h-8 w-auto">
      <Switch id={id} checked={checked} onCheckedChange={onCheckedChange} />
      <FieldLabel
        htmlFor={id}
        className="text-sm tracking-normal normal-case text-foreground"
      >
        {children}
      </FieldLabel>
    </Field>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex h-8 items-center gap-2">
      <span className="w-14 shrink-0 text-xs tracking-wide text-muted-foreground uppercase">
        {label}
      </span>
      <div className="flex min-w-0 flex-1 items-center gap-1">{children}</div>
    </div>
  );
}

/** An input whose value lands on Enter or blur, not on every keystroke. */
function CommitInput({
  value,
  onCommit,
  ...props
}: {
  value: string;
  onCommit: (value: string) => void;
} & Omit<
  React.ComponentProps<typeof Input>,
  "value" | "onChange" | "onBlur" | "onKeyDown"
>) {
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft !== null && draft !== value) onCommit(draft);
    setDraft(null);
  };
  return (
    <Input
      {...props}
      value={draft ?? value}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          commit();
        }
      }}
    />
  );
}

function ReadOnlyField({ value }: { value: string }) {
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      toast.success("Copied");
    } catch (error) {
      toast.error("Could not copy", { description: String(error) });
    }
  };
  return (
    <>
      <code className="flex h-8 min-w-0 flex-1 items-center truncate rounded-lg border border-input bg-muted/50 px-2.5 font-mono text-sm text-muted-foreground select-all">
        {value}
      </code>
      <Button
        variant="ghost"
        size="icon"
        aria-label="Copy"
        title="Copy"
        onClick={copy}
      >
        <CopyIcon />
      </Button>
    </>
  );
}
