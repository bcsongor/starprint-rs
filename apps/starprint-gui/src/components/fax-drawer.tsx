import { useState } from "react";
import { CopyIcon, RefreshCwIcon, XIcon } from "lucide-react";
import { toast } from "sonner";
import { CommitInput } from "@/components/commit-input";
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
import {
  byName,
  type FaxContact,
  type FaxLine,
  type FaxRelay,
  type Profile,
} from "@/lib/api";
import { isFaxNumber } from "@/lib/fax-number";

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
  onSettings: (settings: { name: string; printer: string | null }) => void;
  /** Resolves once the relay has answered with its name. */
  onAddRelay: (url: string) => Promise<void>;
  onRemoveRelay: (relay: FaxRelay) => void;
  /** Resolves once the contact is saved, and rejects if it was not. */
  onSaveContact: (number: string, name: string) => Promise<void>;
  onRemoveContact: (contact: FaxContact) => void;
}

/** The server's fax line: its number, who answers and its relays. */
export function FaxDrawer({
  open,
  onOpenChange,
  fax,
  profiles,
  onActivate,
  onReplace,
  onSettings,
  onAddRelay,
  onRemoveRelay,
  onSaveContact,
  onRemoveContact,
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
          <Line fax={fax} onActivate={onActivate} onReplace={onReplace} />
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
          <Contacts
            contacts={fax.contacts}
            onSave={onSaveContact}
            onRemove={onRemoveContact}
          />
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
}: {
  fax: FaxLine;
  onActivate: () => void;
  onReplace: () => void;
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
    return (
      <div className="flex items-center gap-1">
        <div className="min-w-0 flex-1">
          <div className="text-xs tracking-wide text-muted-foreground uppercase">
            Your number
          </div>
          <div className="text-sm break-all select-all">
            <FaxNumber number={number} />
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
        <NewNumber onReplace={onReplace} />
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
            size="icon"
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

/** The fax book: the numbers this line faxes, under names given here. */
function Contacts({
  contacts,
  onSave,
  onRemove,
}: {
  contacts: FaxContact[];
  onSave: (number: string, name: string) => Promise<void>;
  onRemove: (contact: FaxContact) => void;
}) {
  const [name, setName] = useState("");
  const [number, setNumber] = useState("");
  const [saving, setSaving] = useState(false);
  const complete = name.trim() !== "" && isFaxNumber(number);
  const save = async () => {
    if (!complete || saving) return;
    setSaving(true);
    try {
      await onSave(number.trim(), name.trim());
      setName("");
      setNumber("");
    } catch {
      // Reported by the caller; what was typed stays to correct.
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="flex flex-col gap-2">
      <div className="text-xs tracking-wide text-muted-foreground uppercase">
        Fax book
      </div>
      {contacts.length === 0 && (
        <p className="text-sm text-muted-foreground">
          No one yet. Faxes from someone here print under their name.
        </p>
      )}
      <ul className="flex flex-col">
        {contacts.map((contact) => (
          <li key={contact.number} className="flex items-center gap-2 py-1">
            <div className="min-w-0 flex-1">
              <div className="text-sm">{contact.name}</div>
              <div className="truncate text-xs">
                <FaxNumber number={contact.number} />
              </div>
            </div>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Remove ${contact.name}`}
              title="Remove"
              onClick={() => onRemove(contact)}
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
          void save();
        }}
      >
        <Input
          aria-label="Name"
          placeholder="Name"
          autoComplete="off"
          className="w-28 shrink-0"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <Input
          aria-label="Fax number"
          placeholder="*star1…"
          autoComplete="off"
          spellCheck={false}
          className="font-mono text-xs"
          value={number}
          onChange={(e) => setNumber(e.target.value)}
        />
        <Button type="submit" variant="outline" disabled={!complete || saving}>
          {saving ? "Saving…" : "Add"}
        </Button>
      </form>
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
