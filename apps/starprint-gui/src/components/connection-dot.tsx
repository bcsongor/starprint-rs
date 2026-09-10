import type { Status } from "@/hooks/use-statuses";
import { cn } from "@/lib/utils";

/**
 * Lays out a dot and the name it belongs to, at the same distance
 * wherever the pair appears: the profile picker and the API button.
 */
export const DOTTED = "flex items-center gap-1.5";

/** A status dot: green when the printer answers, grey when it does not. */
export function ConnectionDot({
  status,
  label,
  className,
}: {
  status: Status;
  label: string;
  className?: string;
}) {
  return (
    <span
      role="status"
      aria-label={label}
      title={label}
      className={cn(
        // `self-center`: the select's value and item rows are flex
        // containers without an alignment, so a fixed-size dot would
        // otherwise sit at the top edge.
        "size-1.5 shrink-0 self-center rounded-full transition-colors",
        status === "online" && "bg-emerald-500",
        status === "checking" && "animate-pulse bg-muted-foreground/40",
        status === "offline" && "bg-muted-foreground/30",
        className,
      )}
    />
  );
}
