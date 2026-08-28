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

  const updatePrinter = (next: Printer) => {
    setPrinter(next);
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

  const canPrint = card.text.trim().length > 0 && !printing;

  const print = async () => {
    setPrinting(true);
    try {
      const report = await printTaskCard(card, printer);
      toast.success(`Sent ${report.bytes} bytes to ${printer.host}`);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setPrinting(false);
    }
  };

  const showHexdump = async () => {
    try {
      setHexdump(await taskCardHexdump(card, printer));
    } catch (error) {
      toast.error(String(error));
    }
  };

  return (
    <main className="min-h-screen bg-muted/40 p-6">
      <div className="mx-auto grid max-w-5xl gap-6 md:grid-cols-[minmax(0,1fr)_20rem]">
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
              <TaskCardForm card={card} onChange={setCard} />
              <div className="flex gap-2">
                <Button onClick={print} disabled={!canPrint}>
                  <PrinterIcon />
                  {printing ? "Printing…" : "Print"}
                </Button>
                <Button
                  variant="outline"
                  onClick={showHexdump}
                  disabled={!card.text.trim()}
                >
                  <TerminalIcon />
                  Show bytes
                </Button>
              </div>
            </CardContent>
          </Card>

          {hexdump !== null && (
            <Card>
              <CardHeader>
                <CardTitle>Bytes</CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="overflow-x-auto rounded-md bg-muted p-3 text-xs">
                  {hexdump}
                </pre>
              </CardContent>
            </Card>
          )}
        </div>

        <div className="grid gap-6 content-start">
          <PrinterSettings printer={printer} onChange={updatePrinter} />
          <Card>
            <CardHeader>
              <CardTitle>Preview</CardTitle>
            </CardHeader>
            <CardContent>
              <CardPreview layout={layout} kind={printer.kind} />
            </CardContent>
          </Card>
        </div>
      </div>
    </main>
  );
}
