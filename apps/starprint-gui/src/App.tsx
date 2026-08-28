import { useEffect, useState } from "react";
import { format } from "date-fns";
import { CopyIcon, PrinterIcon, TerminalIcon } from "lucide-react";
import { toast } from "sonner";
import { CardPreview } from "@/components/card-preview";
import { PrintOptions } from "@/components/print-options";
import { ProfileToolbar } from "@/components/profile-toolbar";
import { TaskCardForm } from "@/components/task-card-form";
import { TestPageForm } from "@/components/test-page-form";
import { TestPagePreview } from "@/components/test-page-preview";
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
  jobHexdump,
  printJob,
  taskCardLayout,
  testPageSections,
  type HexDump,
  type Job,
  type Layout,
  type Printer,
  type Section,
  type TaskCard,
  type TestPage,
} from "@/lib/api";
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
  { value: "test-page", label: "Test page" },
];

function emptyCard(): TaskCard {
  return { text: "", priority: false, due: format(new Date(), "yyyy-MM-dd") };
}

const DEFAULT_TEST_PAGE: TestPage = { paper: "80", doubleResolution: false };

export default function App() {
  const [profiles, setProfiles] = useState<Profiles>(DEFAULT_PROFILES);
  const [workflow, setWorkflow] = useState<Workflow>("task-card");
  const [card, setCard] = useState<TaskCard>(emptyCard);
  const [testPage, setTestPage] = useState<TestPage>(DEFAULT_TEST_PAGE);
  const [layout, setLayout] = useState<Layout | null>(null);
  const [sections, setSections] = useState<Section[]>([]);
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

  /** Per-job settings are remembered on the active profile. */
  const updatePrinter = (next: Printer) =>
    updateProfiles({
      ...profiles,
      profiles: profiles.profiles.map((p) =>
        p.id === profile.id ? { ...p, ...next } : p,
      ),
    });

  useEffect(() => {
    let cancelled = false;
    taskCardLayout(card, printer.kind)
      .then((result) => {
        if (!cancelled) setLayout(result);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
  }, [card, printer.kind]);

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

  const job: Job =
    workflow === "task-card"
      ? { kind: "task-card", ...card }
      : { kind: "test-page", ...testPage };
  const ready = workflow !== "task-card" || card.text.trim().length > 0;
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
    <main className="flex min-h-screen flex-col">
      {/* The toolbar is rendered in the dark theme so it reads as app
          chrome; every control inside picks up the dark tokens. */}
      <header className="dark flex items-end gap-3 border-b bg-background px-4 py-3 text-foreground">
        <ProfileToolbar
          state={profiles}
          onChange={updateProfiles}
          printing={printing}
        />
        <PrintOptions printer={printer} onChange={updatePrinter} />
      </header>

      {/* The window's minimum size is chosen so this never has to wrap. */}
      <div className="grid flex-1 grid-cols-[minmax(0,1.25fr)_minmax(0,1fr)]">
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
          ) : (
            <TestPageForm
              page={testPage}
              kind={printer.kind}
              onChange={setTestPage}
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

        <section className="flex flex-col gap-4 bg-muted p-4">
          <Label render={<h2 />} className="h-5">
            Preview
            {workflow === "task-card" && layout && (
              <span className="font-normal tracking-normal normal-case">
                {layout.columns} columns
              </span>
            )}
          </Label>
          {workflow === "task-card" ? (
            <CardPreview layout={layout} kind={printer.kind} />
          ) : (
            <TestPagePreview sections={sections} />
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
