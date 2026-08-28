import { useEffect, useState } from "react";
import { PrinterIcon, TerminalIcon } from "lucide-react";
import { toast } from "sonner";
import { CardPreview } from "@/components/card-preview";
import { PrinterSettings } from "@/components/printer-settings";
import { TaskCardForm } from "@/components/task-card-form";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  printTaskCard,
  taskCardHexdump,
  taskCardLayout,
  type Layout,
  type Printer,
  type TaskCard,
} from "@/lib/api";
import { DEFAULT_PRINTER, loadPrinter, savePrinter } from "@/lib/settings";

const EMPTY_CARD: TaskCard = { text: "", priority: false, due: null };

export default function App() {
  const [printer, setPrinter] = useState<Printer>(DEFAULT_PRINTER);
  const [card, setCard] = useState<TaskCard>(EMPTY_CARD);
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
    <main className="min-h-screen bg-muted/40 p-6">
      <div className="mx-auto grid max-w-6xl gap-6 lg:grid-cols-2">
        <div className="grid gap-6 content-start">
          <Card>
            <CardHeader>
              <CardTitle>Task card</CardTitle>
              <CardDescription>
                A bold, double-size task with an optional priority flag and due
                date.
              </CardDescription>
            </CardHeader>
            <CardContent className="grid gap-6">
              <TaskCardForm
                card={card}
                onChange={updateCard}
                onSubmit={print}
              />
              <div className="flex flex-wrap items-center gap-2">
                <Button onClick={print} disabled={!canPrint}>
                  <PrinterIcon />
                  {printing ? "Printing…" : "Print"}
                </Button>
                <Button
                  variant="outline"
                  onClick={toggleHexdump}
                  disabled={!hasText}
                >
                  <TerminalIcon />
                  {hexdump === null ? "Show bytes" : "Hide bytes"}
                </Button>
                {!hasHost && (
                  <span className="text-destructive text-xs">
                    Enter the printer's host to print.
                  </span>
                )}
              </div>
            </CardContent>
          </Card>

          <PrinterSettings printer={printer} onChange={updatePrinter} />
        </div>

        <div className="grid gap-6 content-start lg:sticky lg:top-6">
          <Card>
            <CardHeader>
              <CardTitle>Preview</CardTitle>
              <CardDescription>
                {layout
                  ? `${layout.columns} columns, as laid out for the ${printer.kind} printer.`
                  : "Loading…"}
              </CardDescription>
            </CardHeader>
            <CardContent>
              <CardPreview layout={layout} kind={printer.kind} />
            </CardContent>
          </Card>

          {hexdump !== null && (
            <Card>
              <CardHeader>
                <CardTitle>Bytes</CardTitle>
                <CardDescription>
                  Exactly what Print sends, for checking without a printer.
                </CardDescription>
              </CardHeader>
              <CardContent>
                <pre className="overflow-x-auto rounded-md bg-muted p-3 text-xs leading-relaxed">
                  {hexdump}
                </pre>
              </CardContent>
            </Card>
          )}
        </div>
      </div>
    </main>
  );
}
