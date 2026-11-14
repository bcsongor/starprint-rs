import { useEffect, useLayoutEffect, useRef } from "react";
import Bold from "@tiptap/extension-bold";
import Document from "@tiptap/extension-document";
import Paragraph from "@tiptap/extension-paragraph";
import TextNode from "@tiptap/extension-text";
import { Placeholder, UndoRedo } from "@tiptap/extensions";
import { Fragment, Slice } from "@tiptap/pm/model";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import {
  BoldIcon,
  ContrastIcon,
  DropletIcon,
  UnfoldHorizontalIcon,
  UnfoldVerticalIcon,
  type LucideIcon,
} from "lucide-react";
import { SubmitLabel } from "@/components/submit-textarea";
import { Button } from "@/components/ui/button";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import type { PrinterKind, Style, Text } from "@/lib/api";
import { STYLES, sameRuns, styleMark, toDoc, toRuns } from "@/lib/runs";

interface Props {
  text: Text;
  kind: PrinterKind;
  onChange: (text: Text) => void;
  /** Called on Ctrl/Cmd+Enter in the text field. */
  onSubmit: () => void;
}

/**
 * Text with its styles drawn as they print, as far as a screen can.
 * Double height is drawn at twice the size both ways, since a browser
 * cannot narrow a character that is being edited; the preview is exact.
 * A style applies to the selection, or with none to what is typed next.
 */
export function TextForm({ text, kind, onChange, onSubmit }: Props) {
  // The editor is made once, so its handlers read the latest props here.
  const props = useRef({ onChange, onSubmit });
  useLayoutEffect(() => {
    props.current = { onChange, onSubmit };
  });

  const editor = useEditor({
    extensions: [
      Document,
      Paragraph,
      TextNode,
      Bold,
      styleMark("wide"),
      styleMark("tall"),
      styleMark("accent"),
      UndoRedo,
      Placeholder.configure({ placeholder: "Anything you want on paper" }),
    ],
    content: toDoc(text.runs),
    autofocus: "end",
    editorProps: {
      attributes: {
        role: "textbox",
        "aria-multiline": "true",
        "aria-labelledby": "text-label",
        class: "outline-none",
      },
      handleKeyDown: (_, event) => {
        if (event.key !== "Enter" || !(event.ctrlKey || event.metaKey)) {
          return false;
        }
        props.current.onSubmit();
        return true;
      },
      // A line to a paragraph both ways, where ProseMirror would fold
      // blank lines on paste and double every break on copy. Pasted text
      // takes the style typed text would: the one chosen for it, or the
      // one it lands in.
      clipboardTextParser: (pasted, $context, _, view) => {
        const { schema, storedMarks } = view.state;
        const marks = storedMarks ?? $context.marks();
        const lines = pasted
          .split(/\r\n?|\n/)
          .map((line) =>
            schema.nodes.paragraph.create(
              null,
              line ? schema.text(line, marks) : null,
            ),
          );
        return new Slice(Fragment.from(lines), 1, 1);
      },
      clipboardTextSerializer: (slice) =>
        slice.content.textBetween(0, slice.content.size, "\n"),
    },
    onUpdate: ({ editor }) =>
      props.current.onChange({ runs: toRuns(editor.state.doc) }),
  });

  // A job loaded from a schedule replaces what is being edited; what the
  // editor handed up itself leaves it, and the cursor, alone.
  useEffect(() => {
    if (!sameRuns(text.runs, toRuns(editor.state.doc))) {
      editor.commands.setContent(toDoc(text.runs), { emitUpdate: false });
    }
  }, [editor, text.runs]);

  // The selection's styles, or with none what typing would take.
  const active = useEditorState({
    editor,
    selector: ({ editor }) =>
      Object.fromEntries(
        STYLES.map((style) => [style, editor.isActive(style)]),
      ) as Record<Style, boolean>,
  });

  const styles: Record<Style, { label: string; Icon: LucideIcon }> = {
    bold: { label: "Bold", Icon: BoldIcon },
    wide: { label: "Wide", Icon: UnfoldHorizontalIcon },
    tall: { label: "Tall", Icon: UnfoldVerticalIcon },
    // A thermal head has no second colour, so it inverts instead.
    accent:
      kind === "impact"
        ? { label: "Red", Icon: DropletIcon }
        : { label: "Inverse", Icon: ContrastIcon },
  };

  return (
    <FieldGroup className="gap-4">
      <Field>
        <SubmitLabel
          id="text-label"
          render={<span />}
          onClick={() => editor.commands.focus("end")}
        >
          Text
        </SubmitLabel>
        <EditorContent
          editor={editor}
          data-kind={kind}
          // Grows with the text up to what the shortest window fits.
          // The editable fills the box, so a click anywhere in it lands.
          className="printed-editor flex max-h-48 min-h-28 w-full cursor-text flex-col overflow-y-auto rounded-lg border border-border bg-background px-2.5 py-2 font-mono text-sm transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 dark:bg-input/30 [&>div]:flex-1"
        />
      </Field>

      <Field>
        <FieldLabel render={<span />}>Style</FieldLabel>
        {/* Buttons, not toggles: a toggle's tint is too quiet to read. */}
        <div className="flex items-center gap-2">
          {STYLES.map((style) => {
            const { label, Icon } = styles[style];
            return (
              <Button
                key={style}
                variant={active[style] ? "default" : "secondary"}
                className="px-2.5"
                aria-pressed={active[style]}
                // Keeps the selection the style is for.
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => editor.chain().focus().toggleMark(style).run()}
              >
                <Icon />
                {label}
              </Button>
            );
          })}
        </div>
      </Field>
    </FieldGroup>
  );
}
