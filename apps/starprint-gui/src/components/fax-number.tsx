import { runs, type Tint } from "@/lib/fax-number";
import { cn } from "@/lib/utils";

const COLOURS: Record<Tint, string> = {
  plain: "",
  amber: "text-amber-600 dark:text-amber-400",
  teal: "text-teal-600 dark:text-teal-400",
  violet: "text-violet-600 dark:text-violet-400",
};

/**
 * A fax number as people read it: `*star1`, then its runs (see
 * `lib/fax-number`), each a chunk of its own colour with a gap before
 * it, so the gaps and the colours mark the same boundaries. The gaps
 * are margins, not spaces, so a number copied from here pastes whole.
 * Text that is not a number is shown as it is.
 */
export function FaxNumber({
  number,
  className,
}: {
  number: string;
  className?: string;
}) {
  const cut = runs(number);
  if (!cut) return <span className={cn("font-mono", className)}>{number}</span>;
  return (
    <span className={cn("font-mono", className)}>
      *star1
      {cut.map((run, i) => (
        <span key={i} className={cn("ml-[0.5ch]", COLOURS[run.tint])}>
          {run.text}
        </span>
      ))}
    </span>
  );
}
