import type { ReactNode } from "react";
import { PaperSheet } from "@/components/paper-sheet";
import { Label } from "@/components/ui/label";
import type { Profile } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  hint?: string | null;
  printer?: Profile;
  children: ReactNode;
}

/** The preview heading and, for printable content, its sheet of paper. */
export function PreviewPane({ hint, printer, children }: Props) {
  return (
    <section
      className={cn(
        "flex min-h-0 flex-col gap-4 bg-muted p-4",
        // A sheet can briefly overflow while its scale catches up to a new size.
        printer && "overflow-hidden",
      )}
    >
      <Label render={<h2 />} className="h-5">
        Preview
        {hint && (
          <span className="font-normal tracking-normal normal-case">
            {hint}
          </span>
        )}
      </Label>
      {printer ? <PaperSheet printer={printer}>{children}</PaperSheet> : children}
    </section>
  );
}

/** What the pane says when there is nothing to draw. */
export function PreviewNote({
  children,
  error,
}: {
  children: ReactNode;
  error?: boolean;
}) {
  return (
    <p
      className={cn(
        "py-[1em] text-center text-xl",
        error ? "text-red-700" : "text-black/30",
      )}
    >
      {children}
    </p>
  );
}
