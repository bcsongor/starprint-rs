import { useEffect, useState } from "react";
import { CopyIcon } from "lucide-react";
import { toast } from "sonner";
import { ConnectionDot, DOTTED } from "@/components/connection-dot";
import { OptionSelect } from "@/components/option-select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Toggle } from "@/components/ui/toggle";
import type { ApiServer } from "@/hooks/use-api-server";
import { listAddresses, type Address, type Listen } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  enabled: boolean;
  /** Where the API is listening and what it asks for, once it is. */
  server: ApiServer | null;
  /** Where it listens, or will. */
  listen: Listen;
  onChange: (enabled: boolean) => void;
  onListenChange: (listen: Listen) => void;
  className?: string;
}

/**
 * The HTTP API's button. Off, a click starts the server. On, it is
 * pressed in with a lit dot, and a click opens the details: where to
 * listen, the URL and token to copy, and Stop. The dot is the one the
 * profiles show, so the two read as the same signal. The serving
 * itself is `useApiServer`.
 */
export function ApiToggle({
  enabled,
  server,
  listen,
  onChange,
  onListenChange,
  className,
}: Props) {
  const [open, setOpen] = useState(false);
  const [addresses, setAddresses] = useState<Address[]>([]);
  // What is being typed, or null when the field shows the port in use.
  // Applied on Enter or blur once it is a port; otherwise dropped.
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
    <Popover
      open={enabled && open}
      onOpenChange={(next) => {
        if (!enabled) onChange(true);
        setOpen(next);
      }}
    >
      <PopoverTrigger
        render={
          <Toggle
            variant="outline"
            pressed={enabled}
            title={enabled ? "HTTP API" : "Serve the HTTP API to other programs"}
            aria-label="Serve the HTTP API"
            className={cn(
              DOTTED,
              // Grey out, green in. Hover from either side is the green
              // border over a dimmer green, so the grey button previews on
              // and the green one dips before it goes.
              "bg-secondary aria-pressed:border-emerald-500/40 aria-pressed:bg-emerald-500/15",
              "hover:border-emerald-500/40 hover:bg-emerald-500/10",
              "aria-pressed:hover:border-emerald-500/40 aria-pressed:hover:bg-emerald-500/10",
              className,
            )}
          />
        }
      >
        <ConnectionDot
          status={enabled ? "online" : "offline"}
          label={enabled ? "Serving" : "Off"}
        />
        API
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-auto min-w-80 gap-0 overflow-hidden p-0"
      >
        <div className="border-b bg-muted p-2.5 [&_[data-slot=select-trigger]]:bg-background [&_[data-slot=select-value]]:font-mono">
          <Row label="Listen">
            <OptionSelect
              id="api-address"
              value={listen.ip}
              options={options}
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
            <ReadOnlyField value={server?.url ?? null} placeholder="Starting…" />
          </Row>
          <Row label="Token">
            <ReadOnlyField value={server?.token ?? null} />
          </Row>
          <Button
            variant="outline"
            size="sm"
            className="self-end"
            onClick={() => {
              onChange(false);
              setOpen(false);
            }}
          >
            Stop
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

/** A label column of one width, so the rows line up. */
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

/**
 * A value in a box the size of the select, greyed to say it is read
 * only, with a button that copies it.
 */
function ReadOnlyField({
  value,
  placeholder,
}: {
  value: string | null;
  placeholder?: string;
}) {
  const copy = async () => {
    if (value === null) return;
    try {
      await navigator.clipboard.writeText(value);
      toast.success("Copied");
    } catch (error) {
      toast.error("Could not copy", { description: String(error) });
    }
  };
  return (
    <>
      <code
        className="flex h-8 min-w-0 flex-1 items-center truncate rounded-lg border border-input bg-muted/50 px-2.5 font-mono text-xs text-muted-foreground select-all"
      >
        {value ?? placeholder}
      </code>
      <Button
        variant="ghost"
        size="icon"
        aria-label="Copy"
        title="Copy"
        disabled={value === null}
        onClick={copy}
      >
        <CopyIcon />
      </Button>
    </>
  );
}
