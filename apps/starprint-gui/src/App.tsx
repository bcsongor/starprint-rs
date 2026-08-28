import { useEffect, useState } from "react";
import { format } from "date-fns";
import { PrinterIcon, TerminalIcon } from "lucide-react";
import { toast } from "sonner";
import { CardPreview } from "@/components/card-preview";
import { PrinterSettings } from "@/components/printer-settings";
import { TaskCardForm } from "@/components/task-card-form";
import { Button } from "@/components/ui/button";
import {
  printTaskCard,
  taskCardHexdump,
  taskCardLayout,
  type Layout,
  type Printer,
  type TaskCard,
} from "@/lib/api";
import { DEFAULT_PRINTER, loadPrinter, savePrinter } from "@/lib/settings";

const HEADING =
  "text-[11px] font-medium uppercase tracking-wider text-muted-foreground";

function emptyCard(): TaskCard {
  return { text: "", priority: false, due: format(new Date(), "yyyy-MM-dd") };
}

export default function App() {
  const [printer, setPrinter] = useState<Printer>(DEFAULT_PRINTER);
  const [card, setCard] = useState<TaskCard>(emptyCard);
  const [layout, setLayout] = useState<Layout | null>(null);
  const [hexdump, setHexdump] = useState<string | null>(null);
  const [printing, setPrinting] = useState(false);

  useEffect(() => {
    loadPrinter().then(setPrinter).catch(console.error);
  }, []);

  // The hex dump is a snapshot; drop it whenever the inputs change so it
  // never shows stale bytes.
  const updateCard = (next: TaskCard) => {
    setCard(next);
    setHexdump(null);
  };

  const updatePrinter = (next: Printer) => {
    setPrinter(next);
    setHexdump(null);
    savePrinter(next).catch(console.error);
  };

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
      toast.success(`Printed on ${printer.host}`, {
        description: `${report.bytes} bytes sent.`,
      });
    } catch (error) {
      toast.error("Print failed", { description: String(error) });
    } finally {
      setPrinting(false);
    }
  };

  const toggleHexdump = async () => {
    if (hexdump !== null) {
      setHexdump(null);
      return;
    }
    try {
      setHexdump(await taskCardHexdump(card, printer));
    } catch (error) {
      toast.error("Could not build the job", { description: String(error) });
    }
  };

  return (
    <main className="flex min-h-screen flex-col">
      <header className="border-b bg-card px-3 py-2">
        <PrinterSettings printer={printer} onChange={updatePrinter} />
      </header>

      <div className="grid flex-1 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
        <section className="grid content-start gap-3 border-b p-3 md:border-r md:border-b-0">
          <TaskCardForm card={card} onChange={updateCard} onSubmit={print} />
          <div className="flex flex-wrap items-center gap-1.5 border-t pt-3">
            <Button size="sm" onClick={print} disabled={!canPrint}>
              <PrinterIcon />
              {printing ? "Printing…" : "Print"}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={toggleHexdump}
              disabled={!hasText}
            >
              <TerminalIcon />
              {hexdump === null ? "Bytes" : "Hide bytes"}
            </Button>
            {!hasHost && (
              <span className="text-[11px] text-destructive">
                Enter the printer host to print.
              </span>
            )}
          </div>
          {hexdump !== null && (
            <pre className="overflow-x-auto rounded-md bg-muted px-2 py-1.5 font-mono text-[11px] leading-snug">
              {hexdump}
            </pre>
          )}
        </section>

        <section className="grid content-start gap-3 bg-muted/40 p-3">
          <h2 className={HEADING}>
            Preview
            {layout && (
              <span className="ml-1.5 font-normal normal-case tracking-normal">
                {layout.columns} cols · {printer.kind}
              </span>
            )}
          </h2>
          <CardPreview layout={layout} kind={printer.kind} />
        </section>
      </div>
    </main>
  );
}
