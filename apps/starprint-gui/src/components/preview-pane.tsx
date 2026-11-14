import type { ReactNode } from "react";
import { PaperSheet, PaperStack } from "@/components/paper-sheet";
import { Label } from "@/components/ui/label";
import type { Profile } from "@/lib/api";
import { cn } from "@/lib/utils";

interface Props {
  hint?: string | null;
  printer?: Profile;
  /** Why the server could not preview the job, drawn in its place. */
  error?: string | null;
  children?: ReactNode;
  /** Sheets one under another in a pane that scrolls, in place of
   * `children` on one sheet, for a form that prints several. */
  sheets?: ReactNode[];
}

/** The preview heading and, for printable content, its sheet of paper
 * or a stack of them. */
export function PreviewPane({ hint, printer, error, children, sheets }: Props) {
  const note = error ? <PreviewNote error>{error}</PreviewNote> : null;
  const content = note ?? children;
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
      {!printer ? (
        content
      ) : sheets ? (
        <PaperStack printer={printer}>{note ? [note] : sheets}</PaperStack>
      ) : (
        <PaperSheet printer={printer}>{content}</PaperSheet>
      )}
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
