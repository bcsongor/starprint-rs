import { useState } from "react";
import { SendIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { FaxNumber } from "@/components/fax-number";
import type { FaxContact, FaxLine } from "@/lib/api";
import { isFaxNumber } from "@/lib/fax-number";

interface Props {
  fax: FaxLine | null;
  /** Whether the form holds a job worth sending. */
  ready: boolean;
  /** Resolves once the relay has taken the fax, and rejects if it did not. */
  onSend: (to: string) => Promise<void>;
  /** Opens the fax drawer, for a line that is not set up yet. */
  onSetUp: () => void;
}

/** The contact `to` names, if it names one. */
function named(contacts: FaxContact[], to: string): FaxContact | undefined {
  const name = to.trim().toLowerCase();
  return contacts.find((c) => c.name.toLowerCase() === name);
}

/** Faxes the job in the form: pick someone from the fax book, or paste
 * a number, and press Send. */
export function FaxButton({ fax, ready, onSend, onSetUp }: Props) {
  const [open, setOpen] = useState(false);
  const [to, setTo] = useState("");
  const [sending, setSending] = useState(false);
  const setUp = fax?.number != null && fax.relays.length > 0;
  const contacts = fax?.contacts ?? [];
  const chosen = named(contacts, to);
  const complete = chosen !== undefined || isFaxNumber(to);
  const typed = to.trim().toLowerCase();
  const matches = contacts.filter(
    (c) =>
      c !== chosen &&
      (c.name.toLowerCase().includes(typed) || c.number.includes(typed)),
  );

  const send = async () => {
    if (!complete || sending) return;
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
      <PopoverTrigger
        render={<Button variant="outline" disabled={!ready || !fax} />}
      >
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
              <Button type="submit" disabled={!complete || sending}>
                {sending ? "Sending…" : "Send"}
              </Button>
            </form>
            {(chosen ?? isFaxNumber(to)) && (
              <p className="truncate text-xs">
                <FaxNumber number={chosen?.number ?? to} />
              </p>
            )}
            {matches.length > 0 && (
              <ul className="flex max-h-48 flex-col overflow-y-auto">
                {matches.map((contact) => (
                  <li key={contact.number}>
                    <button
                      type="button"
                      className="flex w-full items-baseline gap-2 rounded-md px-2 py-1 text-left hover:bg-muted"
                      onClick={() => setTo(contact.name)}
                    >
                      <span className="shrink-0">{contact.name}</span>
                      <FaxNumber
                        number={contact.number}
                        className="truncate text-xs"
                      />
                    </button>
                  </li>
                ))}
              </ul>
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
      </PopoverContent>
    </Popover>
  );
}
