import { ConnectionDot, DOTTED } from "@/components/connection-dot";
import { Toggle } from "@/components/ui/toggle";
import { cn } from "@/lib/utils";

interface Props {
  enabled: boolean;
  /** Where the API is listening, once it is. */
  url: string | null;
  onChange: (enabled: boolean) => void;
  className?: string;
}

/**
 * A push button for the HTTP API: pressed in with a lit dot while it
 * serves, out and grey when it does not. The dot is the one the
 * profiles show, so the two read as the same signal. The serving
 * itself is `useApiServer`.
 */
export function ApiToggle({ enabled, url, onChange, className }: Props) {
  return (
    <Toggle
      variant="outline"
      pressed={enabled}
      onPressedChange={onChange}
      title={url ?? "Serve the HTTP API to other programs"}
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
    >
      <ConnectionDot
        status={enabled ? "online" : "offline"}
        label={enabled ? "Serving" : "Off"}
      />
      API
    </Toggle>
  );
}
