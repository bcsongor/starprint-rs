import { useEffect, useMemo, useState } from "react";
import { format } from "date-fns";
import {
  CalendarClockIcon,
  CopyIcon,
  PrinterIcon,
  TerminalIcon,
} from "lucide-react";
import { toast } from "sonner";
import { ApiToggle } from "@/components/api-toggle";
import { CardPreview } from "@/components/card-preview";
import { LinearBar } from "@/components/linear-bar";
import { NoteForm } from "@/components/note-form";
import { NotePreview } from "@/components/note-preview";
import { PictureForm } from "@/components/picture-form";
import { PicturePreview } from "@/components/picture-preview";
import { PrintOptions } from "@/components/print-options";
import { ProfileToolbar } from "@/components/profile-toolbar";
import { QrForm } from "@/components/qr-form";
import { QrPreview } from "@/components/qr-preview";
import { ScheduleDialog, type Draft } from "@/components/schedule-dialog";
import { SchedulesDrawer } from "@/components/schedules-drawer";
import { SchedulesToggle } from "@/components/schedules-toggle";
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
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useApiServer } from "@/hooks/use-api-server";
import { useAutoPrint } from "@/hooks/use-auto-print";
import { useScheduler } from "@/hooks/use-scheduler";
import {
  DEFAULT_NOTE,
  DEFAULT_PICTURE,
  DEFAULT_QR,
  DEFAULT_TEST_PAGE,
  DEFAULT_TEXT,
  TWO_COLOR_DENSITY,
  jobHexdump,
  printJob,
  printScheduled,
  type HexDump,
  type Job,
  type Note,
  type Picture,
  type PrintReport,
  type Printer,
  type Qr,
  type Listen,
  type TaskCard,
  type TestPage,
  type Text,
} from "@/lib/api";
import { NAMES, newSchedule, summary, toScheduled } from "@/lib/schedule";
import {
  DEFAULT_LISTEN,
  DEFAULT_PROFILES,
  activeProfile,
  loadApiEnabled,
  loadApiListen,
  loadLinear,
  loadProfiles,
  loadSchedules,
  saveApiEnabled,
  saveApiListen,
  saveLinear,
  saveProfiles,
  saveSchedules,
  toPrinter,
  type Linear,
  type Profiles,
  type Schedule,
  type Schedules,
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
  const [apiListen, setApiListen] = useState(DEFAULT_LISTEN);
  // Null until loaded with the profiles; the scheduler must not be handed
  // a list missing any of them, or their run record goes with it.
  const [schedules, setSchedules] = useState<Schedules | null>(null);
  const [drawer, setDrawer] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);

  useEffect(() => {
    // Restore the API's profiles and address before enabling it, and the
    // schedules in the same render as their profiles.
    Promise.all([
      loadProfiles(),
      loadApiEnabled(),
      loadApiListen(),
      loadSchedules(),
    ])
      .then(([profiles, enabled, listen, schedules]) => {
        setProfiles(profiles);
        setApiListen(listen);
        setApiEnabled(enabled);
        setSchedules(schedules);
      })
      .catch(console.error);
    loadLinear().then(setLinear).catch(console.error);
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

  const updateApiListen = (next: Listen) => {
    setApiListen(next);
    saveApiListen(next).catch(console.error);
  };

  const updateSchedules = (next: Schedules) => {
    setSchedules(next);
    saveSchedules(next).catch(console.error);
  };

  const apiServer = useApiServer(
    apiEnabled,
    profiles.profiles,
    apiListen,
    () => updateApiEnabled(false),
  );

  useScheduler(schedules, profiles.profiles, () => {
    if (schedules) updateSchedules({ ...schedules, running: false });
  });

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
  const jobs: Record<Workflow, Job> = {
    "task-card": { kind: "task-card", ...card },
    text: { kind: "text", ...text },
    note: { kind: "note", ...note },
    qr: { kind: "qr", ...code },
    "test-page": { kind: "test-page", ...testPage },
    picture: { kind: "picture", ...picture },
  };
  const job = jobs[workflow];
  const ready =
    job.kind === "task-card" || job.kind === "text"
      ? job.text.trim().length > 0
      : job.kind === "qr"
        ? job.data.trim().length > 0
        : job.kind === "picture"
          ? job.path.trim().length > 0
          : true;
  const hasHost = printer.host.trim().length > 0;
  const canPrint = ready && hasHost && !printing;

  /** Runs one print and reports it; `what` names the job, `on` the profile. */
  const send = async (
    what: string,
    on: string,
    print: () => Promise<PrintReport>,
  ) => {
    setPrinting(true);
    try {
      const report = await print();
      toast.success(`Printed ${what} on ${on}`, {
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
    await send(NAMES[workflow], profile.name, () => printJob(job, printer));
  };

  useAutoPrint(linear, (card) =>
    send(card.reference ?? "task card", profile.name, () =>
      printJob({ kind: "task-card", ...card }, printer),
    ),
  );

  /** Puts a scheduled job back in its form, to change and schedule again. */
  const loadJob = (job: Job) => {
    switch (job.kind) {
      case "task-card":
        setCard(job);
        break;
      case "text":
        setText(job);
        break;
      case "note":
        setNote(job);
        break;
      case "qr":
        setCode(job);
        break;
      case "picture":
        setPicture(job);
        break;
      case "test-page":
        setTestPage(job);
        break;
    }
    setWorkflow(job.kind);
  };

  const saveSchedule = (schedule: Schedule) => {
    if (!schedules || !draft) return;
    const items = draft.editing
      ? schedules.items.map((s) => (s.id === schedule.id ? schedule : s))
      : [...schedules.items, schedule];
    // A first schedule is meant to run, so scheduling turns them on.
    updateSchedules({ running: schedules.running || !draft.editing, items });
    setDraft(null);
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
        {/* The API and the schedules serve whichever profile is named,
            so they sit in the picker's column, at the form's right edge
            like the cut switch below. */}
        <div className="flex items-end gap-2 px-4">
          <ProfileToolbar
            state={profiles}
            onChange={updateProfiles}
            printing={printing}
          />
          <ApiToggle
            enabled={apiEnabled}
            server={apiServer}
            listen={apiListen}
            onChange={updateApiEnabled}
            onListenChange={updateApiListen}
          />
          <SchedulesToggle
            running={schedules?.running ?? false}
            onOpen={() => {
              if (schedules && !schedules.running) {
                updateSchedules({ ...schedules, running: true });
              }
              setDrawer(true);
            }}
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
              <Button
                variant="outline"
                disabled={!ready || !schedules}
                onClick={() =>
                  setDraft({
                    schedule: newSchedule(job, profile.id),
                    editing: false,
                  })
                }
              >
                <CalendarClockIcon />
                Schedule
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

        {workflow === "task-card" ? (
          <CardPreview card={card} kind={kind} paper={paper} />
        ) : workflow === "text" ? (
          <TextPreview text={text} kind={kind} paper={paper} />
        ) : workflow === "note" ? (
          <NotePreview note={note} kind={kind} paper={paper} />
        ) : workflow === "qr" ? (
          <QrPreview code={code} kind={kind} paper={paper} />
        ) : workflow === "test-page" ? (
          <TestPagePreview page={testPage} kind={kind} />
        ) : (
          <PicturePreview
            picture={picture}
            kind={kind}
            paper={paper}
          />
        )}
      </div>

      {schedules && (
        <SchedulesDrawer
          open={drawer}
          onOpenChange={setDrawer}
          schedules={schedules}
          onChange={updateSchedules}
          profiles={profiles.profiles}
          onEdit={(schedule) => setDraft({ schedule, editing: true })}
          onPrint={(schedule) => {
            const scheduled = toScheduled(schedule, profiles.profiles);
            const to = profiles.profiles.find((p) => p.id === schedule.profileId);
            if (scheduled && to) {
              send(summary(schedule.job), to.name, () =>
                printScheduled(scheduled),
              );
            }
          }}
          onLoad={(schedule) => {
            loadJob(schedule.job);
            setDrawer(false);
          }}
        />
      )}

      <ScheduleDialog
        draft={draft}
        profiles={profiles.profiles}
        onClose={() => setDraft(null)}
        onSave={saveSchedule}
      />

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
