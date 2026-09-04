import { useEffect, useMemo, useState } from "react";
import { format } from "date-fns";
import { CopyIcon, PrinterIcon, TerminalIcon } from "lucide-react";
import { toast } from "sonner";
import { ApiToggle } from "@/components/api-toggle";
import { CardPreview } from "@/components/card-preview";
import { LinearBar } from "@/components/linear-bar";
import { NoteForm } from "@/components/note-form";
import { NotePreview } from "@/components/note-preview";
import { PaperSheet } from "@/components/paper-sheet";
import { PictureForm } from "@/components/picture-form";
import { PicturePreview } from "@/components/picture-preview";
import { PrintOptions } from "@/components/print-options";
import { ProfileToolbar } from "@/components/profile-toolbar";
import { QrForm } from "@/components/qr-form";
import { QrPreview } from "@/components/qr-preview";
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
import { useApiServer } from "@/hooks/use-api-server";
import { useAsync } from "@/hooks/use-async";
import { useAutoPrint } from "@/hooks/use-auto-print";
import { usePreview } from "@/hooks/use-preview";
import {
  DEFAULT_NOTE,
  DEFAULT_PICTURE,
  DEFAULT_QR,
  DEFAULT_TEST_PAGE,
  DEFAULT_TEXT,
  QUIET_MODULES,
  TWO_COLOR_DENSITY,
  jobHexdump,
  noteLayout,
  picturePreview,
  printJob,
  qrLayout,
  qrPreview,
  taskCardLayout,
  testPageSections,
  textLayout,
  type HexDump,
  type Job,
  type Note,
  type Picture,
  type Printer,
  type Qr,
  type TaskCard,
  type TestPage,
  type Text,
} from "@/lib/api";
import { roll } from "@/lib/paper";
import {
  DEFAULT_PROFILES,
  activeProfile,
  loadApiEnabled,
  loadLinear,
  loadProfiles,
  saveApiEnabled,
  saveLinear,
  saveProfiles,
  toPrinter,
  type Linear,
  type Profiles,
} from "@/lib/settings";

type Workflow = Job["kind"];

/** The everyday jobs; the test page is a diagnostic and sits apart. */
const WORKFLOWS: { value: Workflow; label: string }[] = [
  { value: "task-card", label: "Task" },
  { value: "text", label: "Text" },
  { value: "note", label: "Note" },
  { value: "qr", label: "QR" },
  { value: "picture", label: "Picture" },
];

function emptyCard(): TaskCard {
  return {
    text: "",
    priority: false,
    reference: null,
    due: format(new Date(), "yyyy-MM-dd"),
  };
}

/** Names the job in the toast once it is sent. */
const NAMES: Record<Workflow, string> = {
  "task-card": "task card",
  text: "text",
  note: "note slip",
  qr: "QR code",
  picture: "picture",
  "test-page": "test page",
};

export default function App() {
  const [profiles, setProfiles] = useState<Profiles>(DEFAULT_PROFILES);
  const [workflow, setWorkflow] = useState<Workflow>("task-card");
  const [card, setCard] = useState<TaskCard>(emptyCard);
  const [text, setText] = useState<Text>(DEFAULT_TEXT);
  const [note, setNote] = useState<Note>(DEFAULT_NOTE);
  const [code, setCode] = useState<Qr>(DEFAULT_QR);
  const [testPage, setTestPage] = useState<TestPage>(DEFAULT_TEST_PAGE);
  const [pictureSettings, setPicture] = useState<Picture>(DEFAULT_PICTURE);
  const [hexdump, setHexdump] = useState<HexDump | null>(null);
  const [printing, setPrinting] = useState(false);
  const [linear, setLinear] = useState<Linear | null>(null);
  const [apiEnabled, setApiEnabled] = useState(false);

  useEffect(() => {
    loadProfiles().then(setProfiles).catch(console.error);
    loadLinear().then(setLinear).catch(console.error);
    loadApiEnabled().then(setApiEnabled).catch(console.error);
  }, []);

  const updateProfiles = (next: Profiles) => {
    setProfiles(next);
    saveProfiles(next).catch(console.error);
  };

  const updateLinear = (next: Linear) => {
    setLinear(next);
    saveLinear(next).catch(console.error);
  };

  const updateApiEnabled = (next: boolean) => {
    setApiEnabled(next);
    saveApiEnabled(next).catch(console.error);
  };

  const apiUrl = useApiServer(apiEnabled, profiles.profiles, () =>
    updateApiEnabled(false),
  );

  const profile = activeProfile(profiles);
  const printer = toPrinter(profile);
  const twoColor =
    printer.kind === "thermal" && printer.density === TWO_COLOR_DENSITY;
  const picture = useMemo(
    () => (twoColor ? { ...pictureSettings, double: false } : pictureSettings),
    [pictureSettings, twoColor],
  );

  const updatePrinter = (next: Printer) =>
    updateProfiles({
      ...profiles,
      profiles: profiles.profiles.map((p) =>
        p.id === profile.id ? { ...p, ...next } : p,
      ),
    });

  const { kind, paper } = printer;
  const layout = useAsync(
    () => taskCardLayout(card, kind, paper),
    [card, kind, paper],
  );
  const wrap = useAsync(
    () => textLayout(text, kind, paper),
    [text, kind, paper],
  );
  const slip = useAsync(
    () => noteLayout(note, kind, paper),
    [note, kind, paper],
  );
  const sections = useAsync(
    () => testPageSections(testPage, kind),
    [testPage, kind],
  );

  // An empty string encodes to a perfectly valid symbol, which is not
  // one the preview should show.
  const hasData = code.data.trim().length > 0;
  const qr = usePreview(async () => {
    if (!hasData) return null;
    const [value, png] = await Promise.all([
      qrLayout(code, kind, paper),
      qrPreview(code, kind, paper),
    ]);
    return { value, png };
  }, [code, kind, paper]);
  const preview = usePreview(async () => {
    if (picture.path === "") return null;
    return { value: null, png: await picturePreview(picture, kind, paper) };
  }, [picture, kind, paper]);

  /** What the print button sends, whether it can, and the preview's hint. */
  const workflows: Record<
    Workflow,
    { job: Job; ready: boolean; hint: string | null }
  > = {
    "task-card": {
      job: { kind: "task-card", ...card },
      ready: card.text.trim().length > 0,
      hint: layout && `${layout.columns} columns`,
    },
    text: {
      job: { kind: "text", ...text },
      ready: text.text.trim().length > 0,
      hint: wrap && `${wrap.columns} columns`,
    },
    note: {
      job: { kind: "note", ...note },
      ready: true,
      hint: `${note.rows} rows, ${note.rows * note.pitch} mm`,
    },
    qr: {
      job: { kind: "qr", ...code },
      ready: qr.value !== null,
      // The slider sets the symbol; the block on paper is that plus the
      // quiet zone, which is what this measures.
      hint:
        qr.value &&
        `${qr.value.modules - 2 * QUIET_MODULES} modules, ${Math.round(qr.value.widthMm)} mm`,
    },
    "test-page": {
      job: { kind: "test-page", ...testPage },
      ready: true,
      hint: null,
    },
    picture: {
      job: { kind: "picture", ...picture },
      ready: preview.url !== null,
      hint: `${roll(kind, paper).dots} dots`,
    },
  };
  const { job, ready, hint } = workflows[workflow];
  const hasHost = printer.host.trim().length > 0;
  const canPrint = ready && hasHost && !printing;

  /** Sends one job to the active profile and reports it; `what` names it. */
  const send = async (job: Job, what: string) => {
    setPrinting(true);
    try {
      const report = await printJob(job, printer);
      toast.success(`Printed ${what} on ${profile.name}`, {
        description: `${report.bytes} bytes sent.`,
      });
    } catch (error) {
      toast.error(`Could not print ${what}`, { description: String(error) });
    } finally {
      setPrinting(false);
    }
  };

  const print = async () => {
    if (!canPrint) return;
    await send(job, NAMES[workflow]);
  };

  useAutoPrint(linear, (card) =>
    send({ kind: "task-card", ...card }, card.reference ?? "task card"),
  );

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
        {/* The API serves whichever profile a caller names, so it sits
            in the picker's column, at the form's right edge like the
            cut switch below. */}
        <div className="flex items-end px-4">
          <ProfileToolbar
            state={profiles}
            onChange={updateProfiles}
            printing={printing}
          />
          <ApiToggle
            enabled={apiEnabled}
            url={apiUrl}
            onChange={updateApiEnabled}
            className="ml-auto"
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
              {/* Reached for when something looks wrong, not to print
                  with, so it sits behind a divider and reads quieter. */}
              <span aria-hidden className="mx-1 h-4 w-px bg-border" />
              <TabsTrigger value="test-page" className="text-foreground/40">
                Test page
              </TabsTrigger>
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
          ) : workflow === "qr" ? (
            <QrForm code={code} onChange={setCode} onSubmit={print} />
          ) : workflow === "test-page" ? (
            <TestPageForm
              page={testPage}
              kind={printer.kind}
              onChange={setTestPage}
            />
          ) : (
            <PictureForm
              picture={pictureSettings}
              printer={printer}
              onChange={setPicture}
            />
          )}

          <div className="mt-auto flex flex-col gap-4">
            {/* Linear feeds task cards, so it sits with that tab; the
              polling itself runs whichever tab is showing. */}
            {workflow === "task-card" && (
              <LinearBar linear={linear} onChange={updateLinear} />
            )}

            <div className="flex flex-wrap items-center gap-2 border-t pt-4">
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
            <TestPagePreview sections={sections ?? []} />
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
              ) : workflow === "qr" ? (
                <QrPreview layout={qr.value} url={qr.url} error={qr.error} />
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
