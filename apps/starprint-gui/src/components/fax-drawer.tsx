import { useState } from "react";
import { CopyIcon, QrCodeIcon, RefreshCwIcon, XIcon } from "lucide-react";
import { toast } from "sonner";
import { ConnectionDot } from "@/components/connection-dot";
import { OptionSelect } from "@/components/option-select";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { FaxNumber } from "@/components/fax-number";
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
  /** Mines a new number to replace the current one. */
  onReplace: () => void;
  /** Puts the number in the QR code form, to print for someone to scan. */
  onShowCode: (number: string) => void;
  onSettings: (settings: { printer: string | null }) => void;
  /** Resolves once the relay has answered with its name. */
  onAddRelay: (url: string) => Promise<void>;
  onRemoveRelay: (relay: FaxRelay) => void;
}

/** The server's fax line: its number, where faxes print and its relays. */
export function FaxDrawer({
  open,
  onOpenChange,
  fax,
  profiles,
  onActivate,
  onReplace,
  onShowCode,
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
          <Line
            fax={fax}
            onActivate={onActivate}
            onReplace={onReplace}
            onShowCode={onShowCode}
          />

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
                onSettings({ printer: printer === FIRST ? null : printer })
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

function Line({
  fax,
  onActivate,
  onReplace,
  onShowCode,
}: {
  fax: FaxLine;
  onActivate: () => void;
  onReplace: () => void;
  onShowCode: (number: string) => void;
}) {
  if (fax.number) {
    const number = fax.number;
    const copy = async () => {
      try {
        await navigator.clipboard.writeText(number);
        toast.success("Copied");
      } catch (error) {
        toast.error("Could not copy", { description: String(error) });
      }
    };
    // The actions sit beside the label, so the number has the full
    // width and fits on one line.
    return (
      <div className="flex flex-col gap-1">
        <div className="flex items-center">
          <div className="flex-1 text-xs tracking-wide text-muted-foreground uppercase">
            Your number
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="Copy"
            title="Copy"
            onClick={copy}
          >
            <CopyIcon />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="QR code"
            title="QR code"
            onClick={() => onShowCode(number)}
          >
            <QrCodeIcon />
          </Button>
          <NewNumber onReplace={onReplace} />
        </div>
        <div className="text-sm break-all select-all">
          <FaxNumber number={number} />
        </div>
      </div>
    );
  }
  return (
    <div className="flex flex-col items-start gap-2">
      <Button onClick={onActivate}>Activate line</Button>
      <p className="text-xs text-muted-foreground">
        The number is the address of this server's key.
      </p>
    </div>
  );
}

/** Makes a new number, once the user has seen what it costs: the old
 * number stops reaching this printer. */
function NewNumber({ onReplace }: { onReplace: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="New number"
            title="New number"
          />
        }
      >
        <RefreshCwIcon />
      </PopoverTrigger>
      <PopoverContent align="end">
        <p className="font-medium">Get a new number?</p>
        <p className="text-muted-foreground">
          Anyone who has your current number will no longer reach you.
        </p>
        <Button
          size="sm"
          className="self-start"
          onClick={() => {
            setOpen(false);
            onReplace();
          }}
        >
          New number
        </Button>
      </PopoverContent>
    </Popover>
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
