import { useState } from "react";
import { CopyIcon, XIcon } from "lucide-react";
import { toast } from "sonner";
import { CommitInput } from "@/components/commit-input";
import { ConnectionDot } from "@/components/connection-dot";
import { OptionSelect } from "@/components/option-select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { byName, type FaxLine, type FaxRelay, type Profile } from "@/lib/api";

/** Stands for "no printer chosen", which prints on the first profile. No
 * profile's name has a slash in it. */
const FIRST = "/";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fax: FaxLine;
  profiles: Profile[];
  onActivate: () => void;
  onSettings: (settings: { name: string; printer: string | null }) => void;
  /** Resolves once the relay has answered with its name. */
  onAddRelay: (url: string) => Promise<void>;
  onRemoveRelay: (relay: FaxRelay) => void;
}

/** The server's fax line: its number, who answers and its relays. */
export function FaxDrawer({
  open,
  onOpenChange,
  fax,
  profiles,
  onActivate,
  onSettings,
  onAddRelay,
  onRemoveRelay,
}: Props) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="gap-0 sm:max-w-md">
        <SheetHeader className="border-b">
          <SheetTitle>Fax</SheetTitle>
          <SheetDescription>
            Printing to another printer by its number, end-to-end
            encrypted.
          </SheetDescription>
        </SheetHeader>
        <div className="flex flex-col gap-4 overflow-y-auto p-4">
          <Line fax={fax} onActivate={onActivate} />
          <Labelled label="Name" htmlFor="fax-name">
            <CommitInput
              id="fax-name"
              placeholder="Shown to senders"
              value={fax.name}
              onCommit={(name) => onSettings({ name, printer: fax.printer })}
            />
          </Labelled>
          <Labelled label="Prints on" htmlFor="fax-printer">
            <OptionSelect
              id="fax-printer"
              value={fax.printer ?? FIRST}
              options={[
                { value: FIRST, label: "First printer" },
                ...byName(profiles).map((p) => ({
                  value: p.name,
                  label: p.name,
                })),
              ]}
              onChange={(printer) =>
                onSettings({
                  name: fax.name,
                  printer: printer === FIRST ? null : printer,
                })
              }
              align="start"
              labelClassName="w-40"
            />
          </Labelled>
          <Relays
            relays={fax.relays}
            onAdd={onAddRelay}
            onRemove={onRemoveRelay}
          />
        </div>
        <p className="mt-auto border-t p-4 text-sm text-muted-foreground">
          Every fax to this number prints.
        </p>
      </SheetContent>
    </Sheet>
  );
}

function Line({ fax, onActivate }: { fax: FaxLine; onActivate: () => void }) {
  if (fax.number) {
    const copy = async () => {
      try {
        await navigator.clipboard.writeText(fax.number ?? "");
        toast.success("Copied");
      } catch (error) {
        toast.error("Could not copy", { description: String(error) });
      }
    };
    return (
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <div className="text-xs tracking-wide text-muted-foreground uppercase">
            Your number
          </div>
          <div className="font-mono text-2xl tabular-nums select-all">
            {fax.number}
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Copy"
          title="Copy"
          onClick={copy}
        >
          <CopyIcon />
        </Button>
      </div>
    );
  }
  if (fax.activation) {
    // Mining is luck, so the bar stops short of the end rather than
    // promising a finish it cannot know.
    const share = Math.min(fax.activation.tried / fax.activation.expected, 0.95);
    return (
      <div className="flex flex-col gap-2">
        <div className="text-sm">Activating…</div>
        <div className="h-1.5 overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full bg-primary transition-[width]"
            style={{ width: `${share * 100}%` }}
          />
        </div>
        <p className="text-xs text-muted-foreground">
          A few minutes, once.
        </p>
      </div>
    );
  }
  return (
    <div className="flex flex-col items-start gap-2">
      <Button onClick={onActivate}>Activate line</Button>
      <p className="text-xs text-muted-foreground">
        A few minutes, once. The number is bound to this server's key.
      </p>
    </div>
  );
}

function Relays({
  relays,
  onAdd,
  onRemove,
}: {
  relays: FaxRelay[];
  onAdd: (url: string) => Promise<void>;
  onRemove: (relay: FaxRelay) => void;
}) {
  const [url, setUrl] = useState("");
  const [adding, setAdding] = useState(false);
  const add = async () => {
    if (!url.trim() || adding) return;
    setAdding(true);
    try {
      await onAdd(url.trim());
      setUrl("");
    } catch {
      // Reported by the caller; the address stays to correct.
    } finally {
      setAdding(false);
    }
  };
  return (
    <div className="flex flex-col gap-2">
      <div className="text-xs tracking-wide text-muted-foreground uppercase">
        Relays
      </div>
      {relays.length === 0 && (
        <p className="text-sm text-muted-foreground">
          None yet. Faxes travel through relays.
        </p>
      )}
      <ul className="flex flex-col">
        {relays.map((relay) => (
          <li key={relay.name} className="flex items-center gap-2 py-1">
            <ConnectionDot
              status={
                relay.online === null
                  ? "checking"
                  : relay.online
                    ? "online"
                    : "offline"
              }
              label={relay.problem ?? relay.url}
            />
            <div className="min-w-0 flex-1">
              <div className="font-mono text-sm">{relay.name}</div>
              <div className="truncate text-xs text-muted-foreground">
                {relay.problem ?? relay.url}
              </div>
            </div>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Remove ${relay.name}`}
              title="Remove"
              onClick={() => onRemove(relay)}
            >
              <XIcon />
            </Button>
          </li>
        ))}
      </ul>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <Input
          aria-label="Relay address"
          placeholder="https://relay.example.com"
          autoComplete="off"
          className="font-mono"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
        />
        <Button type="submit" variant="outline" disabled={!url.trim() || adding}>
          {adding ? "Adding…" : "Add"}
        </Button>
      </form>
    </div>
  );
}

function Labelled({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label
        htmlFor={htmlFor}
        className="text-xs tracking-wide text-muted-foreground uppercase"
      >
        {label}
      </label>
      {children}
    </div>
  );
}
