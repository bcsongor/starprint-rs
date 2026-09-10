import { useEffect, useState } from "react";
import { CopyIcon } from "lucide-react";
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
import type { Server } from "@/lib/api";
import { listAddresses, type Address, type Listen } from "@/lib/embedded";

interface Props {
  server: Server;
  /** Whether the server also listens on the LAN, at `listen`. */
  lan: boolean;
  listen: Listen;
  onLanChange: (lan: boolean) => void;
  onListenChange: (listen: Listen) => void;
}

/** Where the server is and what it wants, for programs that print through it. */
export function ApiButton({
  server,
  lan,
  listen,
  onLanChange,
  onListenChange,
}: Props) {
  const [open, setOpen] = useState(false);
  const [addresses, setAddresses] = useState<Address[]>([]);
  // Commit valid port edits on Enter or blur.
  const [draft, setDraft] = useState<string | null>(null);
  const port = draft ?? String(listen.port);
  const applyPort = () => {
    const next = Number(port);
    if (Number.isInteger(next) && next >= 1 && next <= 65535 && next !== listen.port) {
      onListenChange({ ...listen, port: next });
    }
    setDraft(null);
  };

  // Read each time the details open, so a network joined since shows.
  useEffect(() => {
    if (!open) return;
    listAddresses().then(setAddresses).catch(console.error);
  }, [open]);

  // The stored address may belong to a network this machine has left;
  // keep it listed so the select can still show it.
  const options = (
    addresses.some((a) => a.ip === listen.ip)
      ? addresses
      : [...addresses, { ip: listen.ip, name: "not on this machine" }]
  ).map((a) => ({ value: a.ip, label: a.ip, hint: a.name }));

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
          status="online"
          label={lan ? "Serving on the LAN" : "Serving on this machine"}
        />
        API
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-auto min-w-80 gap-0 overflow-hidden p-0"
      >
        <div className="flex flex-col gap-2 border-b bg-muted p-2.5 [&_[data-slot=select-trigger]]:bg-background [&_[data-slot=select-value]]:font-mono">
          <Field orientation="horizontal" className="h-8 w-auto">
            <Switch id="api-lan" checked={lan} onCheckedChange={onLanChange} />
            <FieldLabel
              htmlFor="api-lan"
              className="text-sm tracking-normal normal-case text-foreground"
            >
              Also listen on the LAN
            </FieldLabel>
          </Field>
          <Row label="Listen">
            <OptionSelect
              id="api-address"
              value={listen.ip}
              options={options}
              disabled={!lan}
              onChange={(ip) => onListenChange({ ...listen, ip })}
              align="start"
              labelClassName="w-32 font-mono"
            />
            <Input
              id="api-port"
              aria-label="Port"
              title="Port"
              inputMode="numeric"
              value={port}
              onChange={(e) => setDraft(e.target.value.replace(/\D/g, ""))}
              onBlur={applyPort}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  applyPort();
                }
              }}
              className="w-18 shrink-0 bg-background text-center font-mono"
            />
          </Row>
        </div>
        <div className="flex flex-col gap-2 p-2.5">
          <Row label="URL">
            <ReadOnlyField value={server.url} />
          </Row>
          <Row label="Token">
            <ReadOnlyField value={server.token} />
          </Row>
        </div>
      </PopoverContent>
    </Popover>
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
      <code className="flex h-8 min-w-0 flex-1 items-center truncate rounded-lg border border-input bg-muted/50 px-2.5 font-mono text-xs text-muted-foreground select-all">
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
