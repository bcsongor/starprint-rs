import { useEffect, useRef, useState } from "react";
import { format } from "date-fns";
import { CopyIcon, PrinterIcon, TerminalIcon } from "lucide-react";
import { toast } from "sonner";
import { CardPreview } from "@/components/card-preview";
import { NoteForm } from "@/components/note-form";
import { NotePreview } from "@/components/note-preview";
import { PaperSheet } from "@/components/paper-sheet";
import { PictureForm } from "@/components/picture-form";
import { PicturePreview } from "@/components/picture-preview";
import { PrintOptions } from "@/components/print-options";
import { ProfileToolbar } from "@/components/profile-toolbar";
import { TaskCardForm } from "@/components/task-card-form";
import { TestPageForm } from "@/components/test-page-form";
import { TestPagePreview } from "@/components/test-page-preview";
import { TextForm } from "@/components/text-form";
import { TextPreview } from "@/components/text-preview";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  DEFAULT_NOTE,
  DEFAULT_PICTURE,
  DEFAULT_TEXT,
  jobHexdump,
  noteLayout,
  picturePreview,
  printJob,
  taskCardLayout,
  testPageSections,
  textLayout,
  type HexDump,
  type Job,
  type Layout,
  type Note,
  type NoteLayout,
  type Picture,
  type Printer,
  type Section,
  type TaskCard,
  type TestPage,
  type Text,
  type TextLayout,
} from "@/lib/api";
import { roll } from "@/lib/paper";
import {
  DEFAULT_PROFILES,
  activeProfile,
  loadProfiles,
  saveProfiles,
  toPrinter,
  type Profiles,
} from "@/lib/settings";

type Workflow = Job["kind"];

const WORKFLOWS: { value: Workflow; label: string }[] = [
  { value: "task-card", label: "Task card" },
  { value: "text", label: "Text" },
  { value: "note", label: "Note" },
  { value: "picture", label: "Picture" },
  { value: "test-page", label: "Test page" },
];

function emptyCard(): TaskCard {
  return { text: "", priority: false, due: format(new Date(), "yyyy-MM-dd") };
}

const DEFAULT_TEST_PAGE: TestPage = { doubleResolution: false };

/** Coalesces slider drags; the preview is never more than one render behind. */
const PREVIEW_DEBOUNCE_MS = 30;

export default function App() {
  const [profiles, setProfiles] = useState<Profiles>(DEFAULT_PROFILES);
  const [workflow, setWorkflow] = useState<Workflow>("task-card");
  const [card, setCard] = useState<TaskCard>(emptyCard);
  const [text, setText] = useState<Text>(DEFAULT_TEXT);
  const [note, setNote] = useState<Note>(DEFAULT_NOTE);
  const [testPage, setTestPage] = useState<TestPage>(DEFAULT_TEST_PAGE);
  const [layout, setLayout] = useState<Layout | null>(null);
  const [wrap, setWrap] = useState<TextLayout | null>(null);
  const [slip, setSlip] = useState<NoteLayout | null>(null);
  const [sections, setSections] = useState<Section[]>([]);
  const [picture, setPicture] = useState<Picture>(DEFAULT_PICTURE);
  const [pictureUrl, setPictureUrl] = useState<string | null>(null);
  const [pictureError, setPictureError] = useState<string | null>(null);
  const [hexdump, setHexdump] = useState<HexDump | null>(null);
  const [printing, setPrinting] = useState(false);

  useEffect(() => {
    loadProfiles().then(setProfiles).catch(console.error);
  }, []);

  const updateProfiles = (next: Profiles) => {
    setProfiles(next);
    saveProfiles(next).catch(console.error);
  };

  const profile = activeProfile(profiles);
  const printer = toPrinter(profile);

  const updatePrinter = (next: Printer) =>
    updateProfiles({
      ...profiles,
      profiles: profiles.profiles.map((p) =>
        p.id === profile.id ? { ...p, ...next } : p,
      ),
    });

  useEffect(() => {
    let cancelled = false;
    taskCardLayout(card, printer.kind, printer.paper)
      .then((result) => {
        if (!cancelled) setLayout(result);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [card, printer.kind, printer.paper]);

  useEffect(() => {
    let cancelled = false;
    textLayout(text, printer.kind, printer.paper)
      .then((result) => {
        if (!cancelled) setWrap(result);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [text, printer.kind, printer.paper]);

  useEffect(() => {
    let cancelled = false;
    noteLayout(note, printer.kind, printer.paper)
      .then((result) => {
        if (!cancelled) setSlip(result);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [note, printer.kind, printer.paper]);

  useEffect(() => {
    let cancelled = false;
    testPageSections(testPage, printer.kind)
      .then((result) => {
        if (!cancelled) setSections(result);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [testPage, printer.kind]);

  const previewChain = useRef<Promise<void>>(Promise.resolve());
  useEffect(() => {
    if (!picture.path) return;
    let cancelled = false;
    let url: string | null = null;
    const timer = setTimeout(() => {
      // One render at a time; a stale request is skipped.
      previewChain.current = previewChain.current.then(async () => {
        if (cancelled) return;
        try {
          const png = await picturePreview(
            picture,
            printer.kind,
            printer.paper,
          );
          if (cancelled) return;
          url = URL.createObjectURL(new Blob([png], { type: "image/png" }));
          setPictureUrl(url);
          setPictureError(null);
        } catch (error) {
          if (cancelled) return;
          setPictureUrl(null);
          setPictureError(String(error));
        }
      });
    }, PREVIEW_DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      if (url) URL.revokeObjectURL(url);
    };
  }, [picture, printer.kind, printer.paper]);

  const job: Job =
    workflow === "task-card"
      ? { kind: "task-card", ...card }
      : workflow === "text"
        ? { kind: "text", ...text }
        : workflow === "note"
          ? { kind: "note", ...note }
          : workflow === "test-page"
            ? { kind: "test-page", ...testPage }
            : { kind: "picture", ...picture };
  // Clearing the picture leaves the last preview in state.
  const preview = picture.path
    ? { url: pictureUrl, error: pictureError }
    : { url: null, error: null };
  const ready =
    workflow === "task-card"
      ? card.text.trim().length > 0
      : workflow === "text"
        ? text.text.trim().length > 0
        : workflow === "picture"
          ? preview.url !== null
          : true;
  const hint =
    workflow === "task-card"
      ? layout && `${layout.columns} columns`
      : workflow === "text"
        ? wrap && `${wrap.columns} columns`
        : workflow === "note"
          ? `${note.rows} rows, ${note.rows * note.pitch} mm`
          : workflow === "picture"
            ? `${roll(printer.kind, printer.paper).dots} dots`
            : null;
  const hasHost = printer.host.trim().length > 0;
  const canPrint = ready && hasHost && !printing;

  const print = async () => {
    if (!canPrint) return;
    setPrinting(true);
    try {
      const report = await printJob(job, printer);
      toast.success(`Printed on ${profile.name}`, {
        description: `${report.bytes} bytes sent.`,
      });
    } catch (error) {
      toast.error("Print failed", { description: String(error) });
    } finally {
      setPrinting(false);
    }
  };

  const copyHexdump = async () => {
    if (!hexdump) return;
    try {
      await navigator.clipboard.writeText(hexdump.dump);
      toast.success("Copied", {
        description: `${hexdump.bytes} bytes as a hex dump.`,
      });
    } catch (error) {
      toast.error("Could not copy", { description: String(error) });
    }
  };

  const showHexdump = async () => {
    try {
      setHexdump(await jobHexdump(job, printer));
    } catch (error) {
      toast.error("Could not build the job", { description: String(error) });
    }
  };

  return (
    // `--col` is shared with the toolbar so the print options start where
    // the preview does. Wide enough for the due row on a Wednesday.
    <main className="flex h-screen flex-col [--col:27.5rem]">
      {/* Dark so it reads as app chrome. */}
      <header className="dark grid grid-cols-[var(--col)_minmax(0,1fr)] items-end border-b bg-background py-3 text-foreground">
        <div className="pl-4">
          <ProfileToolbar
            state={profiles}
            onChange={updateProfiles}
            printing={printing}
          />
        </div>
        {/* Paper label starts where the Preview label does. */}
        <div className="grid grid-cols-3 items-end gap-3 px-4">
          <PrintOptions printer={printer} onChange={updatePrinter} />
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[var(--col)_minmax(0,1fr)]">
        <section className="flex flex-col gap-4 border-r p-4">
          <Tabs
            value={workflow}
            onValueChange={(value) => setWorkflow(value as Workflow)}
          >
            <TabsList>
              {WORKFLOWS.map((w) => (
                <TabsTrigger key={w.value} value={w.value}>
                  {w.label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>

          {workflow === "task-card" ? (
            <TaskCardForm card={card} onChange={setCard} onSubmit={print} />
          ) : workflow === "text" ? (
            <TextForm
              text={text}
              kind={printer.kind}
              onChange={setText}
              onSubmit={print}
            />
          ) : workflow === "note" ? (
            <NoteForm note={note} onChange={setNote} />
          ) : workflow === "test-page" ? (
            <TestPageForm
              page={testPage}
              kind={printer.kind}
              onChange={setTestPage}
            />
          ) : (
            <PictureForm
              picture={picture}
              kind={printer.kind}
              onChange={setPicture}
            />
          )}

          <div className="mt-auto flex flex-wrap items-center gap-2 border-t pt-4">
            <Button onClick={print} disabled={!canPrint}>
              <PrinterIcon />
              {printing ? "Printing…" : "Print"}
            </Button>
            <Button variant="outline" onClick={showHexdump} disabled={!ready}>
              <TerminalIcon />
              Bytes
            </Button>
            {!hasHost && (
              <span className="text-sm text-destructive">No host set.</span>
            )}
            <Field orientation="horizontal" className="ml-auto w-auto">
              <Switch
                id="cut"
                checked={printer.cut}
                onCheckedChange={(cut) => updatePrinter({ ...printer, cut })}
              />
              <FieldLabel
                htmlFor="cut"
                className="text-sm tracking-normal normal-case text-foreground"
              >
                Auto cut
              </FieldLabel>
            </Field>
          </div>
        </section>

        <section className="flex min-h-0 flex-col gap-4 bg-muted p-4">
          <Label render={<h2 />} className="h-5">
            Preview
            {hint && (
              <span className="font-normal tracking-normal normal-case">
                {hint}
              </span>
            )}
          </Label>
          {workflow === "test-page" ? (
            <TestPagePreview sections={sections} />
          ) : (
            <PaperSheet kind={printer.kind} paper={printer.paper}>
              {workflow === "task-card" ? (
                <CardPreview layout={layout} kind={printer.kind} />
              ) : workflow === "text" ? (
                <TextPreview layout={wrap} text={text} kind={printer.kind} />
              ) : workflow === "note" ? (
                <NotePreview
                  layout={slip}
                  note={note}
                  kind={printer.kind}
                  paper={printer.paper}
                />
              ) : (
                <PicturePreview url={preview.url} error={preview.error} />
              )}
            </PaperSheet>
          )}
        </section>
      </div>

      <Dialog
        open={hexdump !== null}
        onOpenChange={(open) => {
          if (!open) setHexdump(null);
        }}
      >
        <DialogContent className="w-fit max-w-none sm:max-w-none">
          <DialogHeader>
            <DialogTitle>Bytes</DialogTitle>
            <DialogDescription>
              {hexdump?.bytes} bytes, 16 per row.
            </DialogDescription>
          </DialogHeader>
          <pre className="max-h-[60vh] w-max select-text overflow-y-auto rounded-md bg-muted px-3 py-2 font-mono text-xs leading-relaxed">
            {hexdump?.dump}
          </pre>
          <DialogFooter>
            <Button variant="outline" onClick={copyHexdump}>
              <CopyIcon />
              Copy
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </main>
  );
}
