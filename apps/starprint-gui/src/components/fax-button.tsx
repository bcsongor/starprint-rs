import { useState } from "react";
import { SendIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import type { FaxLine } from "@/lib/api";

interface Props {
  fax: FaxLine | null;
  /** Whether the form holds a job worth sending. */
  ready: boolean;
  /** Resolves once the relay has taken the fax, and rejects if it did not. */
  onSend: (to: string) => Promise<void>;
  /** Opens the fax drawer, for a line that is not set up yet. */
  onSetUp: () => void;
}

/** Shows what is typed as a number reads: `*7441 720938`. */
function asNumber(typed: string): string {
  const digits = typed.replace(/\D/g, "").slice(0, 10);
  if (!digits) return "";
  return `*${digits.slice(0, 4)}${digits.length > 4 ? ` ${digits.slice(4)}` : ""}`;
}

/** Faxes the job in the form: type a number, press Send. */
export function FaxButton({ fax, ready, onSend, onSetUp }: Props) {
  const [open, setOpen] = useState(false);
  const [number, setNumber] = useState("");
  const [sending, setSending] = useState(false);
  const setUp = fax?.number != null && fax.relays.length > 0;
  const complete = number.replace(/\D/g, "").length === 10;

  const send = async () => {
    if (!complete || sending) return;
    setSending(true);
    try {
      await onSend(number);
      setOpen(false);
      setNumber("");
    } catch {
      // Reported by the caller; the number stays to try again.
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
      <PopoverContent align="start" side="top" className="w-72">
        {setUp ? (
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void send();
            }}
          >
            <Input
              autoFocus
              aria-label="Fax number"
              placeholder="*7441 720938"
              inputMode="numeric"
              autoComplete="off"
              className="font-mono tabular-nums"
              value={number}
              onChange={(e) => setNumber(asNumber(e.target.value))}
            />
            <Button type="submit" disabled={!complete || sending}>
              {sending ? "Sending…" : "Send"}
            </Button>
          </form>
        ) : (
          <div className="flex flex-col items-start gap-2">
            <p className="text-sm text-muted-foreground">
              {fax?.number
                ? "No relay."
                : "Line not active."}
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
