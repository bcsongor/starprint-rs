import { useEffect, useEffectEvent } from "react";
import { toast } from "sonner";
import type { TaskCard } from "@/lib/api";
import { assignedIssues, toCard } from "@/lib/linear";
import type { Linear } from "@/lib/settings";

const POLL_MS = 10_000;

/**
 * While auto-print is on, asks Linear every 10 seconds for the open
 * issues assigned to the user and prints a card for each one that was
 * not there last time. The first poll only takes stock, so switching
 * the toggle on, or opening the app with it on, prints nothing by
 * itself. The set is replaced whole each time, so an issue closed and
 * reopened, or reassigned away and back, prints again: it has landed on
 * the desk again.
 */
export function useAutoPrint(
  linear: Linear | null,
  print: (card: TaskCard) => Promise<void>,
) {
  const printCard = useEffectEvent(print);
  const apiKey = linear?.user && linear.autoPrint ? linear.apiKey : null;

  useEffect(() => {
    if (!apiKey) return;
    let cancelled = false;
    let busy = false;
    let failing = false;
    let seen: Set<string> | null = null;

    const poll = async () => {
      if (busy) return;
      busy = true;
      try {
        const issues = await assignedIssues(apiKey);
        if (cancelled) return;
        failing = false;
        const previous = seen;
        const fresh = previous
          ? issues.filter((issue) => !previous.has(issue.id))
          : [];
        // Counted as seen before printing, so a card that fails is
        // reported once and not sent again to a printer that may have
        // taken part of it.
        seen = new Set(issues.map((issue) => issue.id));
        for (const issue of fresh) {
          if (cancelled) return;
          await printCard(toCard(issue));
        }
      } catch (error) {
        // Once per outage, not once per poll.
        if (!failing && !cancelled) {
          toast.error("Linear is not answering", {
            description: String(error),
          });
        }
        failing = true;
      } finally {
        busy = false;
      }
    };

    poll();
    const timer = setInterval(poll, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [apiKey]);
}
