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
      className={cn(DOTTED, className)}
    >
      <ConnectionDot
        status={enabled ? "online" : "offline"}
        label={enabled ? "Serving" : "Off"}
      />
      API
    </Toggle>
  );
}
