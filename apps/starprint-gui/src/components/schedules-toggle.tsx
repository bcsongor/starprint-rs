import { CalendarClockIcon } from "lucide-react";
import { ConnectionDot, DOTTED } from "@/components/connection-dot";
import { Toggle } from "@/components/ui/toggle";
import { cn } from "@/lib/utils";

interface Props {
  running: boolean;
  /** Called on every press; the caller turns schedules on if they are off. */
  onOpen: () => void;
}

export function SchedulesToggle({ running, onOpen }: Props) {
  return (
    <Toggle
      variant="outline"
      pressed={running}
      aria-label="Schedules"
      title={running ? "Schedules" : "Run the schedules"}
      className={cn(
        DOTTED,
        "bg-secondary aria-pressed:border-emerald-500/40 aria-pressed:bg-emerald-500/15",
        "hover:border-emerald-500/40 hover:bg-emerald-500/10",
        "aria-pressed:hover:border-emerald-500/40 aria-pressed:hover:bg-emerald-500/10",
      )}
      onClick={onOpen}
    >
      <ConnectionDot
        status={running ? "online" : "offline"}
        label={running ? "Running" : "Off"}
      />
      <CalendarClockIcon />
    </Toggle>
  );
}
