import { Mark, mergeAttributes, type JSONContent } from "@tiptap/core";
import type { Node } from "@tiptap/pm/model";
import type { Run, Style } from "@/lib/api";

/** In the order a style switches on, which is the order marks nest. */
export const STYLES: readonly Style[] = ["bold", "wide", "tall", "accent"];

/** A style the head has and HTML does not, as a span `index.css` draws.
 * Bold is TipTap's own. */
export const styleMark = (name: Exclude<Style, "bold">) =>
  Mark.create({
    name,
    parseHTML: () => [{ tag: `span[data-${name}]` }],
    renderHTML: ({ HTMLAttributes }) => [
      "span",
      mergeAttributes(HTMLAttributes, { [`data-${name}`]: "" }),
      0,
    ],
  });

const sameStyle = (a: Partial<Run>, b: Partial<Run>) =>
  STYLES.every((style) => !!a[style] === !!b[style]);

/** Runs in order, a style that is off left out and a run merged into
 * the one before when their styles match. */
function push(runs: Run[], text: string, style: Partial<Run>) {
  const last = runs.at(-1);
  if (last && sameStyle(last, style)) last.text += text;
  else runs.push({ text, ...style });
}

/** The editor's document as runs: a paragraph is a printed line, and a
 * text node's marks are its style. */
export function toRuns(doc: Node): Run[] {
  const runs: Run[] = [];
  doc.forEach((paragraph, _, index) => {
    // A line break has no style of its own, so it joins whatever run is
    // before it and text in one style stays one run.
    if (index > 0) push(runs, "\n", runs.at(-1) ?? {});
    paragraph.forEach((node) =>
      push(
        runs,
        node.text ?? "",
        Object.fromEntries(node.marks.map((mark) => [mark.type.name, true])),
      ),
    );
  });
  return runs;
}

/** `runs` as the editor's document, a paragraph to a line. */
export function toDoc(runs: Run[]): JSONContent {
  const lines: JSONContent[][] = [[]];
  for (const run of runs) {
    const marks = STYLES.filter((style) => run[style]).map((type) => ({
      type,
    }));
    run.text.split("\n").forEach((text, index) => {
      if (index > 0) lines.push([]);
      if (text) lines[lines.length - 1].push({ type: "text", text, marks });
    });
  }
  return {
    type: "doc",
    content: lines.map((content) => ({ type: "paragraph", content })),
  };
}

/** Whether two lists of runs print the same, whichever way they spell a
 * style that is off. */
export function sameRuns(a: Run[], b: Run[]): boolean {
  return (
    a.length === b.length &&
    a.every(
      (run, index) => run.text === b[index].text && sameStyle(run, b[index]),
    )
  );
}
