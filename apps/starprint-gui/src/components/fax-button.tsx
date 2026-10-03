import { useRef, useState } from "react";
import { PlusIcon, SendIcon, XIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { FaxNumber } from "@/components/fax-number";
import type { FaxContact, FaxLine, FaxRecent } from "@/lib/api";
import { isFaxNumber } from "@/lib/fax-number";

interface Props {
  fax: FaxLine | null;
  /** Whether the form holds a job worth sending. */
  ready: boolean;
  /** Resolves once the relay has taken the fax, and rejects if it did not. */
  onSend: (to: string) => Promise<void>;
  /** Opens the fax drawer, for a line that is not set up yet. */
  onSetUp: () => void;
  /** Resolves once the contact is saved, and rejects if it was not. */
  onSaveContact: (number: string, name: string) => Promise<void>;
  onRemoveContact: (contact: FaxContact) => void;
}

/** The contact `to` names, if it names one. */
function named(contacts: FaxContact[], to: string): FaxContact | undefined {
  const name = to.trim().toLowerCase();
  return contacts.find((c) => c.name.toLowerCase() === name);
}

/** Faxes the job in the form: pick someone from the fax book, or paste
 * a number, and press Send. The fax book is kept here too, where it is
 * used. */
export function FaxButton({
  fax,
  ready,
  onSend,
  onSetUp,
  onSaveContact,
  onRemoveContact,
}: Props) {
  const [open, setOpen] = useState(false);
  const [to, setTo] = useState("");
  const [sending, setSending] = useState(false);
  const setUp = fax?.number != null && fax.relays.length > 0;
  const contacts = fax?.contacts ?? [];
  const chosen = named(contacts, to);
  const complete = chosen !== undefined || isFaxNumber(to);
  const typed = to.trim().toLowerCase();
  // Typing narrows the book; a name typed in full still lists its entry.
  const listed = contacts.filter(
    (c) => c.name.toLowerCase().includes(typed) || c.number.includes(typed),
  );
  const recent = (fax?.recent ?? []).filter((r) => r.number.includes(typed));

  const send = async () => {
    if (!complete || !ready || sending) return;
    setSending(true);
    try {
      await onSend(to.trim());
      setOpen(false);
      setTo("");
    } catch {
      // Reported by the caller; what was typed stays to try again.
    } finally {
      setSending(false);
    }
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger render={<Button variant="outline" disabled={!fax} />}>
        <SendIcon />
        Fax
      </PopoverTrigger>
      <PopoverContent align="start" side="top" className="w-96">
        {setUp ? (
          <>
            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                void send();
              }}
            >
              <Input
                autoFocus
                aria-label="Name or fax number"
                placeholder="A name, or *star1…"
                autoComplete="off"
                spellCheck={false}
                value={to}
                onChange={(e) => setTo(e.target.value)}
              />
              <Button
                type="submit"
                disabled={!complete || !ready || sending}
                title={ready ? undefined : "Nothing to fax yet."}
              >
                {sending ? "Sending…" : "Send"}
              </Button>
            </form>
            {(chosen ?? isFaxNumber(to)) && (
              <p className="truncate text-xs">
                <FaxNumber number={chosen?.number ?? to} />
              </p>
            )}
          </>
        ) : (
          <div className="flex flex-col items-start gap-2">
            <p className="text-sm text-muted-foreground">
              {fax?.number ? "No relay." : "Line not active."}
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setOpen(false);
                onSetUp();
              }}
            >
              Set up fax…
            </Button>
          </div>
        )}
        <FaxBook
          contacts={listed}
          recent={recent}
          empty={contacts.length === 0}
          onChoose={setTo}
          onSave={onSaveContact}
          onRemove={onRemoveContact}
        />
      </PopoverContent>
    </Popover>
  );
}

/** The fax book: the numbers this line faxes, under names given here,
 * then recent numbers that have none yet. Choosing one fills it in
 * above; adding a recent one fills in the form below. */
function FaxBook({
  contacts,
  recent,
  empty,
  onChoose,
  onSave,
  onRemove,
}: {
  /** Those to list, which typing above may have narrowed. */
  contacts: FaxContact[];
  recent: FaxRecent[];
  /** Whether the book itself has no one in it. */
  empty: boolean;
  /** Takes a name or a number. */
  onChoose: (to: string) => void;
  onSave: (number: string, name: string) => Promise<void>;
  onRemove: (contact: FaxContact) => void;
}) {
  const [name, setName] = useState("");
  const [number, setNumber] = useState("");
  const [saving, setSaving] = useState(false);
  const nameInput = useRef<HTMLInputElement>(null);
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
    <div className="flex flex-col gap-1.5 border-t pt-2.5">
      <div className="text-xs tracking-wide text-muted-foreground uppercase">
        Fax book
      </div>
      {empty && (
        <p className="text-sm text-muted-foreground">
          No one yet. A fax prints under the name given here.
        </p>
      )}
      {/* A pressed button drops a pixel. Without the padding below, the
          last row's would overflow its list, and the scrollbar that
          brings would push the button out from under the pointer. */}
      {contacts.length > 0 && (
        <ul className="flex max-h-48 flex-col overflow-y-auto pb-px">
          {contacts.map((contact) => (
            <li key={contact.number} className="flex items-center">
              <button
                type="button"
                className="flex min-w-0 flex-1 items-baseline gap-2 rounded-md px-2 py-1 text-left hover:bg-muted"
                onClick={() => onChoose(contact.name)}
              >
                <span className="shrink-0">{contact.name}</span>
                <FaxNumber
                  number={contact.number}
                  className="truncate text-xs"
                />
              </button>
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
      )}
      {recent.length > 0 && (
        <>
          <div className="text-xs tracking-wide text-muted-foreground uppercase">
            Recent
          </div>
          <ul className="flex max-h-32 flex-col overflow-y-auto pb-px">
            {recent.map((r) => (
              <li key={r.number} className="flex items-center">
                <button
                  type="button"
                  className="flex min-w-0 flex-1 items-baseline gap-2 rounded-md px-2 py-1 text-left hover:bg-muted"
                  onClick={() => onChoose(r.number)}
                >
                  <FaxNumber number={r.number} className="truncate text-xs" />
                  <span className="ml-auto shrink-0 text-xs text-muted-foreground">
                    {when(r.at)}
                  </span>
                </button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Add to the fax book"
                  title="Add to the fax book"
                  onClick={() => {
                    setNumber(r.number);
                    nameInput.current?.focus();
                  }}
                >
                  <PlusIcon />
                </Button>
              </li>
            ))}
          </ul>
        </>
      )}
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <Input
          ref={nameInput}
          aria-label="Name"
          placeholder="Name"
          autoComplete="off"
          className="w-24 shrink-0"
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

/** Unix seconds as a short local date and time, like `3 Oct, 01:44`. */
function when(at: number): string {
  return new Date(at * 1000).toLocaleString(undefined, {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}
