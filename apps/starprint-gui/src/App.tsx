import { useEffect, useState } from "react";
import { format } from "date-fns";
import { PrinterIcon, TerminalIcon } from "lucide-react";
import { toast } from "sonner";
import { CardPreview } from "@/components/card-preview";
import { PrintOptions } from "@/components/print-options";
import { ProfileToolbar } from "@/components/profile-toolbar";
import { TaskCardForm } from "@/components/task-card-form";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  printTaskCard,
  taskCardHexdump,
  taskCardLayout,
  type HexDump,
  type Layout,
  type Printer,
  type TaskCard,
} from "@/lib/api";
import {
  DEFAULT_PROFILES,
  activeProfile,
  loadProfiles,
  saveProfiles,
  toPrinter,
  type Profiles,
} from "@/lib/settings";

function emptyCard(): TaskCard {
  return { text: "", priority: false, due: format(new Date(), "yyyy-MM-dd") };
}

export default function App() {
  const [profiles, setProfiles] = useState<Profiles>(DEFAULT_PROFILES);
  const [card, setCard] = useState<TaskCard>(emptyCard);
  const [layout, setLayout] = useState<Layout | null>(null);
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

  const hasText = card.text.trim().length > 0;
  const hasHost = printer.host.trim().length > 0;
  const canPrint = hasText && hasHost && !printing;

  const print = async () => {
    if (!canPrint) return;
    setPrinting(true);
    try {
      const report = await printTaskCard(card, printer);
      toast.success(`Printed on ${profile.name}`, {
        description: `${report.bytes} bytes sent.`,
      });
    } catch (error) {
      toast.error("Print failed", { description: String(error) });
    } finally {
      setPrinting(false);
    }
  };

  const showHexdump = async () => {
    try {
      setHexdump(await taskCardHexdump(card, printer));
    } catch (error) {
      toast.error("Could not build the job", { description: String(error) });
    }
  };

  return (
    <main className="flex min-h-screen flex-col">
      {/* The toolbar is rendered in the dark theme so it reads as app
          chrome; every control inside picks up the dark tokens. */}
      <header className="dark flex items-end gap-3 border-b bg-background px-4 py-3 text-foreground">
        <ProfileToolbar state={profiles} onChange={updateProfiles} />
        <PrintOptions printer={printer} onChange={updatePrinter} />
      </header>

      {/* The window's minimum size is chosen so this never has to wrap. */}
      <div className="grid flex-1 grid-cols-[minmax(0,1.25fr)_minmax(0,1fr)]">
        <section className="flex flex-col gap-4 border-r p-4">
          <TaskCardForm card={card} onChange={setCard} onSubmit={print} />
          <div className="flex flex-wrap items-center gap-2 border-t pt-4">
            <Button onClick={print} disabled={!canPrint}>
              <PrinterIcon />
              {printing ? "Printing…" : "Print"}
            </Button>
            <Button variant="outline" onClick={showHexdump} disabled={!hasText}>
              <TerminalIcon />
              Bytes
            </Button>
            {!hasHost && (
              <span className="text-sm text-destructive">
                No host set.
              </span>
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

        <section className="flex flex-col gap-4 bg-muted/40 p-4">
          <Label render={<h2 />} className="h-5">
            Preview
            {layout && (
              <span className="font-normal tracking-normal normal-case">
                {layout.columns} columns
              </span>
            )}
          </Label>
          <CardPreview layout={layout} kind={printer.kind} />
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
          <pre className="max-h-[60vh] w-max overflow-y-auto rounded-md bg-muted px-3 py-2 font-mono text-xs leading-relaxed">
            {hexdump?.dump}
          </pre>
        </DialogContent>
      </Dialog>
    </main>
  );
}
