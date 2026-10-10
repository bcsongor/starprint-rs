import { useMemo, useState } from "react";
import { format } from "date-fns";
import { CalendarClockIcon, PhoneIcon, PrinterIcon } from "lucide-react";
import { toast } from "sonner";
import { ApiButton } from "@/components/api-button";
import { CardPreview } from "@/components/card-preview";
import { FaxButton } from "@/components/fax-button";
import { FaxNumber } from "@/components/fax-number";
import { FaxDrawer } from "@/components/fax-drawer";
import { LinearBar } from "@/components/linear-bar";
import { NoteForm } from "@/components/note-form";
import { NotePreview } from "@/components/note-preview";
import { PictureForm } from "@/components/picture-form";
import { PicturePreview } from "@/components/picture-preview";
import { PreviewPane } from "@/components/preview-pane";
import { PrintOptions } from "@/components/print-options";
import { ProfileToolbar } from "@/components/profile-toolbar";
import { QrForm } from "@/components/qr-form";
import { QrPreview } from "@/components/qr-preview";
import { ScheduleDialog, type Draft } from "@/components/schedule-dialog";
import { SchedulesDrawer } from "@/components/schedules-drawer";
import { TaskCardForm } from "@/components/task-card-form";
import { TestPageForm } from "@/components/test-page-form";
import { TestPagePreview } from "@/components/test-page-preview";
import { TextForm } from "@/components/text-form";
import { TextPreview } from "@/components/text-preview";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAutoPrint } from "@/hooks/use-auto-print";
import { useFax } from "@/hooks/use-fax";
import { usePreview } from "@/hooks/use-preview";
import { useServer } from "@/hooks/use-server";
import {
  DEFAULT_NOTE,
  DEFAULT_PICTURE,
  DEFAULT_QR,
  DEFAULT_TEST_PAGE,
  DEFAULT_TEXT,
  TWO_COLOR_DENSITY,
  byName,
  createClient,
  toSpec,
  type Job,
  type Note,
  type Picture,
  type Profile,
  type Qr,
  type Schedule,
  type Server,
  type TaskCard,
  type TestPage,
  type Text,
} from "@/lib/api";
import { NAMES, newSchedule, summary } from "@/lib/schedule";
import type { Settings } from "@/lib/settings";

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

interface Props {
  /** The server every job, profile and schedule goes through. */
  server: Server;
  /** The one this computer runs, which may or may not be `server`. */
  embedded: Server;
  settings: Settings;
  onSettings: (changes: Partial<Settings>) => void;
}

/** The app once its server is up. */
export function Workspace({ server, embedded, settings, onSettings }: Props) {
  const api = useMemo(() => createClient(server), [server]);
  const { reachable, version, profiles, schedules, status, refresh } =
    useServer(api);
  const [workflow, setWorkflow] = useState<Workflow>("task-card");
  const [card, setCard] = useState<TaskCard>(emptyCard);
  const [text, setText] = useState<Text>(DEFAULT_TEXT);
  const [note, setNote] = useState<Note>(DEFAULT_NOTE);
  const [code, setCode] = useState<Qr>(DEFAULT_QR);
  const [testPage, setTestPage] = useState<TestPage>(DEFAULT_TEST_PAGE);
  const [picture, setPicture] = useState<Picture>(DEFAULT_PICTURE);
  const [pictureFile, setPictureFile] = useState<File | null>(null);
  const [printing, setPrinting] = useState(false);
  const [drawer, setDrawer] = useState(false);
  const { fax, refresh: refreshFax } = useFax(api);
  const [faxDrawer, setFaxDrawer] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);

  /** Runs a change against the server and rejects, after saying what
   * went wrong, for a caller that waits on the outcome. */
  const tryTo = async (what: string, action: () => Promise<void>) => {
    try {
      await action();
    } catch (error) {
      toast.error(what, { description: String(error) });
      throw error;
    }
  };
  /** As `tryTo`, for a caller with nothing to do on failure. */
  const attempt = (what: string, action: () => Promise<void>) =>
    tryTo(what, action).catch(() => {});
  const profile =
    profiles.find((p) => p.name === settings.printer) ?? byName(profiles)[0];

  /** A rename carries the old name's schedules over before the old
   * profile goes, since the server keeps but never runs an orphan. */
  const saveProfile = (next: Profile, replacing: string | null) =>
    attempt("Could not save the profile", async () => {
      await api.putPrinter(next.name, toSpec(next));
      if (replacing) {
        for (const { id, ...spec } of await api.schedules()) {
          if (spec.printer === replacing) {
            await api.replaceSchedule(id, { ...spec, printer: next.name });
          }
        }
        await api.deletePrinter(replacing);
      }
      await refresh();
      onSettings({ printer: next.name });
    });

  const deleteProfile = (name: string) =>
    attempt("Could not delete the profile", async () => {
      await api.deletePrinter(name);
      await refresh();
    });

  const twoColor =
    profile?.kind === "thermal" && profile.density === TWO_COLOR_DENSITY;

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
          ? pictureFile !== null
          : true;
  const canPrint = ready && profile !== undefined && !printing;

  // Empty text still previews, as a placeholder; an empty QR code or a
  // missing picture has nothing to show yet.
  const previewable =
    job.kind === "qr" || job.kind === "picture" ? ready : true;
  const { preview, error } = usePreview(
    api,
    profile?.name ?? null,
    previewable ? job : null,
    job.kind === "picture" ? pictureFile : null,
  );
  /** Sends one job to the active profile and reports it; `what` names it. */
  const send = async (job: Job, image: File | null, what: string) => {
    if (!profile) return;
    setPrinting(true);
    try {
      const report = await api.print(profile.name, job, image);
      toast.success(`Printed ${what} on ${profile.name}`, {
        description: `${report.bytesSent} bytes sent.`,
      });
    } catch (error) {
      toast.error(`Could not print ${what}`, { description: String(error) });
    } finally {
      setPrinting(false);
    }
  };

  const print = async () => {
    if (!canPrint) return;
    await send(job, job.kind === "picture" ? pictureFile : null, NAMES[workflow]);
  };

  /** Faxes the job in the form; rejects, after saying why, if it did
   * not go. */
  const sendFax = (to: string) => {
    const what = NAMES[workflow];
    return tryTo(`Could not fax ${what}`, async () => {
      const sent = await api.sendFax(
        to,
        job,
        job.kind === "picture" ? pictureFile : null,
      );
      toast.success(`Faxed ${what}${sent.name ? ` to ${sent.name}` : ""}`, {
        description: (
          <>
            <FaxNumber number={sent.to} /> via {sent.relay}.
          </>
        ),
      });
    });
  };

  useAutoPrint(settings.linear, (card) =>
    send({ kind: "task-card", ...card }, null, card.reference ?? "task card"),
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

  const saveSchedule = ({ id, ...spec }: Draft) =>
    attempt("Could not save the schedule", async () => {
      if (id) await api.replaceSchedule(id, spec);
      else await api.createSchedule(spec);
      await refresh();
      setDraft(null);
    });

  const deleteSchedule = ({ id }: Schedule) =>
    attempt("Could not delete the schedule", async () => {
      await api.deleteSchedule(id);
      await refresh();
    });

  /** Points every schedule at one printer. The server checks each job
   * against its new printer, so one that does not suit it stays put. */
  const moveSchedules = (printer: string) =>
    attempt("Could not move the schedules", async () => {
      // The list on screen can be ten seconds old.
      const moving = (await api.schedules()).filter(
        (s) => s.printer !== printer,
      );
      const results = await Promise.allSettled(
        moving.map(({ id, ...spec }) =>
          api.replaceSchedule(id, { ...spec, printer }),
        ),
      );
      const failed = results.flatMap((result, i) =>
        result.status === "rejected"
          ? [`${summary(moving[i].job)}: ${String(result.reason)}`]
          : [],
      );
      if (failed.length > 0) {
        toast.error(`Could not move ${failed.length} of ${moving.length} to ${printer}`, {
          description: failed.join("; "),
        });
      } else {
        toast.success(`Moved ${moving.length} to ${printer}`);
      }
      await refresh();
    });

  return (
    // `--col` is shared with the toolbar so the print options start where
    // the preview does. Wide enough for the due row on a Wednesday.
    <main className="flex h-screen flex-col [--col:27.5rem]">
      {/* Dark so it reads as app chrome. */}
      <header className="dark grid grid-cols-[var(--col)_minmax(0,1fr)] items-end border-b bg-background py-3 text-foreground">
        {/* The API and the schedules serve whichever profile is named,
            so they sit in the picker's column, at the form's right edge
            like the cut switch below. The fax line has a printer of its
            own, so it sits apart, before the picker. */}
        <div className="flex items-end gap-2 px-4">
          <Button
            variant="outline"
            size="icon"
            aria-label="Fax"
            title="Fax"
            disabled={!fax}
            onClick={() => setFaxDrawer(true)}
          >
            <PhoneIcon />
          </Button>
          <ProfileToolbar
            profiles={profiles}
            profile={profile}
            status={status}
            onSelect={(printer) => onSettings({ printer })}
            onSave={saveProfile}
            onDelete={deleteProfile}
          />
          <ApiButton
            server={server}
            reachable={reachable}
            version={version}
            embedded={embedded}
            settings={settings}
            onSettings={onSettings}
          />
          <Button
            variant="outline"
            size="icon"
            aria-label="Schedules"
            title="Schedules"
            disabled={!schedules}
            onClick={() => setDrawer(true)}
          >
            <CalendarClockIcon />
          </Button>
        </div>
        {/* Paper label starts where the Preview label does. */}
        <div className="grid grid-cols-3 items-end gap-3 px-4">
          <PrintOptions
            profile={profile}
            onChange={(next) => saveProfile(next, null)}
          />
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
              kind={profile?.kind ?? "thermal"}
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
              kind={profile?.kind ?? "thermal"}
              onChange={setTestPage}
            />
          ) : (
            <PictureForm
              picture={picture}
              file={pictureFile}
              thermal={profile?.kind !== "impact"}
              twoColor={twoColor}
              onChange={setPicture}
              onFile={setPictureFile}
            />
          )}

          <div className="mt-auto flex flex-col gap-4">
            {/* Linear feeds task cards, so it sits with that tab; the
              polling itself runs whichever tab is showing. */}
            {workflow === "task-card" && (
              <LinearBar
                linear={settings.linear}
                onChange={(linear) => onSettings({ linear })}
              />
            )}

            <div className="flex flex-wrap items-center gap-2 border-t pt-4">
              <Button onClick={print} disabled={!canPrint}>
                <PrinterIcon />
                {printing ? "Printing…" : "Print"}
              </Button>
              <Button
                variant="outline"
                disabled={!ready || !schedules || !profile || job.kind === "picture"}
                title={
                  job.kind === "picture"
                    ? "Pictures cannot be scheduled."
                    : undefined
                }
                onClick={() => profile && setDraft(newSchedule(job, profile.name))}
              >
                <CalendarClockIcon />
                Schedule
              </Button>
              <FaxButton
                fax={fax}
                ready={ready}
                onSend={sendFax}
                onSetUp={() => setFaxDrawer(true)}
                onSaveContact={(number, name) =>
                  tryTo("Could not save the contact", async () => {
                    await api.putContact(number, name);
                    await refreshFax();
                  })
                }
                onRemoveContact={(contact) =>
                  attempt("Could not remove the contact", async () => {
                    await api.deleteContact(contact.number);
                    await refreshFax();
                  })
                }
              />
              {!profile && (
                <span className="text-sm text-destructive">No printer.</span>
              )}
              <Field orientation="horizontal" className="ml-auto w-auto">
                <Switch
                  id="cut"
                  checked={profile?.cut ?? true}
                  disabled={!profile}
                  onCheckedChange={(cut) =>
                    profile && saveProfile({ ...profile, cut }, null)
                  }
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

        {/* The preview is the previous job's until the new one lands,
            so each pane takes it only once it is of its own kind. */}
        {!profile ? (
          <PreviewPane>
            <p className="text-sm text-muted-foreground">
              {reachable === "offline"
                ? `${server.url} is not answering.`
                : "Add a printer under Profile actions to begin."}
            </p>
          </PreviewPane>
        ) : workflow === "task-card" ? (
          <CardPreview
            layout={preview?.kind === "task-card" ? preview : null}
            printer={profile}
          />
        ) : workflow === "text" ? (
          <TextPreview
            text={text}
            layout={preview?.kind === "text" ? preview : null}
            printer={profile}
          />
        ) : workflow === "note" ? (
          <NotePreview
            note={note}
            preview={preview?.kind === "note" ? preview : null}
            error={error}
            printer={profile}
          />
        ) : workflow === "qr" ? (
          <QrPreview
            preview={preview?.kind === "qr" ? preview : null}
            error={error}
            printer={profile}
          />
        ) : workflow === "test-page" ? (
          <TestPagePreview
            sections={preview?.kind === "test-page" ? preview.sections : []}
          />
        ) : (
          <PicturePreview
            image={preview?.kind === "picture" ? preview.image : null}
            error={error}
            printer={profile}
          />
        )}
      </div>

      {schedules && (
        <SchedulesDrawer
          open={drawer}
          onOpenChange={setDrawer}
          schedules={schedules}
          profiles={profiles}
          onEnable={({ id, ...spec }, enabled) =>
            attempt("Could not change the schedule", async () => {
              await api.replaceSchedule(id, { ...spec, enabled });
              await refresh();
            })
          }
          onEdit={setDraft}
          onLoad={(schedule) => {
            loadJob(schedule.job);
            setDrawer(false);
          }}
          onDelete={deleteSchedule}
          onMoveAll={moveSchedules}
          clock={
            settings.useRemote
              ? `the clock at ${server.url}`
              : "this computer's clock"
          }
        />
      )}

      {fax && (
        <FaxDrawer
          open={faxDrawer}
          onOpenChange={setFaxDrawer}
          fax={fax}
          profiles={profiles}
          onActivate={() =>
            attempt("Could not activate the line", async () => {
              await api.activateFax();
              await refreshFax();
            })
          }
          onReplace={() =>
            attempt("Could not get a new number", async () => {
              await api.replaceFaxLine();
              await refreshFax();
            })
          }
          onShowCode={(number) => {
            loadJob({ kind: "qr", ...DEFAULT_QR, data: number, caption: number });
            setFaxDrawer(false);
          }}
          onSettings={(changes) =>
            attempt("Could not save the fax settings", async () => {
              await api.putFax(changes);
              await refreshFax();
            })
          }
          onAddRelay={(url) =>
            tryTo("Could not add the relay", async () => {
              const relay = await api.addRelay(url);
              toast.success(`Added ${relay.name}`, { description: relay.url });
              await refreshFax();
            })
          }
          onRemoveRelay={(relay) =>
            attempt("Could not remove the relay", async () => {
              await api.removeRelay(relay.name);
              await refreshFax();
            })
          }
        />
      )}

      <ScheduleDialog
        draft={draft}
        profiles={profiles}
        onClose={() => setDraft(null)}
        onSave={saveSchedule}
      />
    </main>
  );
}
