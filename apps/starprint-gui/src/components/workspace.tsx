import { useMemo, useState } from "react";
import { CalendarClockIcon, PhoneIcon, PrinterIcon } from "lucide-react";
import { toast } from "sonner";
import { ApiButton } from "@/components/api-button";
import { FaxButton } from "@/components/fax-button";
import { FaxNumber } from "@/components/fax-number";
import { FaxDrawer } from "@/components/fax-drawer";
import { JobPreview } from "@/components/job-preview";
import { LinearBar } from "@/components/linear-bar";
import { NoteForm } from "@/components/note-form";
import { PictureForm } from "@/components/picture-form";
import { PreviewPane } from "@/components/preview-pane";
import { PrintOptions } from "@/components/print-options";
import { ProfileToolbar } from "@/components/profile-toolbar";
import { QrForm } from "@/components/qr-form";
import { ScheduleDialog, type Draft } from "@/components/schedule-dialog";
import { SchedulesDrawer } from "@/components/schedules-drawer";
import { TaskCardForm } from "@/components/task-card-form";
import { TestPageForm } from "@/components/test-page-form";
import { TextForm } from "@/components/text-form";
import { Toggle } from "@/components/toggle";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAutoPrint } from "@/hooks/use-auto-print";
import { useJobForm } from "@/hooks/use-job-form";
import { useServer } from "@/hooks/use-server";
import {
  DEFAULT_QR,
  TWO_COLOR_DENSITY,
  createClient,
  toSpec,
  type Job,
  type Profile,
  type Server,
} from "@/lib/api";
import { NAMES, isKind, summary, type Kind } from "@/lib/job";
import { message } from "@/lib/message";
import { newSchedule, repoint, type Refused } from "@/lib/schedule";
import { usesRemote, type Settings } from "@/lib/settings";

/** The everyday jobs; the test page is a diagnostic and sits apart. */
const WORKFLOWS: { value: Kind; label: string }[] = [
  { value: "task-card", label: "Task" },
  { value: "text", label: "Text" },
  { value: "note", label: "Note" },
  { value: "qr", label: "QR" },
  { value: "picture", label: "Picture" },
];

/** Runs a change against the server and rejects, after saying what
 * went wrong, for a caller that waits on the outcome. */
async function tryTo(what: string, action: () => Promise<void>) {
  try {
    await action();
  } catch (error) {
    toast.error(what, { description: message(error) });
    throw error;
  }
}

/** As `tryTo`, for a caller with nothing to do on failure. */
const attempt = (what: string, action: () => Promise<void>) =>
  tryTo(what, action).catch(() => {});

/** Which schedule would not move, and the server's reason. */
const refusal = ({ schedule, reason }: Refused) =>
  `${summary(schedule.job)}: ${message(reason)}`;

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
  const { reachable, version, profiles, schedules, fax, status, refresh } =
    useServer(api);
  const {
    workflow,
    setWorkflow,
    jobs,
    job,
    image,
    ready,
    file,
    setFile,
    put,
    load,
  } = useJobForm();
  // Counted, since a card Linear sends can print while another job does.
  const [printing, setPrinting] = useState(0);
  const [drawer, setDrawer] = useState(false);
  const [faxDrawer, setFaxDrawer] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);

  const remote = usesRemote(settings);
  const profile =
    profiles.find((p) => p.name === settings.printer) ?? profiles.at(0);
  const kind = profile?.kind ?? "thermal";
  const twoColor =
    profile?.kind === "thermal" && profile.density === TWO_COLOR_DENSITY;
  const canPrint = ready && profile !== undefined && printing === 0;

  /** A change to the server, and then what it left, read at once. */
  const thenRefresh = (change: () => Promise<unknown>) => async () => {
    await change();
    await refresh();
  };

  /** A rename carries what named the old profile over before the old
   * one goes: its schedules, since the server keeps but never runs an
   * orphan, and the fax line, which would fall back to the first
   * profile. */
  const saveProfile = (next: Profile, replacing: string | null) =>
    tryTo("Could not save the profile", async () => {
      await api.putPrinter(next.name, toSpec(next));
      if (replacing) {
        const { refused } = await repoint(
          api,
          (s) => s.printer === replacing,
          next.name,
        );
        // The old profile stays for as long as a schedule is left on it.
        if (refused.length > 0) {
          throw new Error(refused.map(refusal).join("; "));
        }
        if (fax?.printer === replacing) {
          await api.putFax({ printer: next.name });
        }
        await api.deletePrinter(replacing);
      }
      await refresh();
      onSettings({ printer: next.name });
    });

  /** As `saveProfile`, for a control that has nothing to do on failure. */
  const changeProfile = (next: Profile) =>
    saveProfile(next, null).catch(() => {});

  /** Sends one job to the active profile and reports it; `what` names it. */
  const send = async (job: Job, image: File | null, what: string) => {
    if (!profile) {
      // Only a card from Linear gets this far without a printer.
      toast.error(`Could not print ${what}`, {
        description: "There is no printer to print it on.",
      });
      return;
    }
    setPrinting((n) => n + 1);
    try {
      const report = await api.print(profile.name, job, image);
      toast.success(`Printed ${what} on ${profile.name}`, {
        description: `${report.bytesSent} bytes sent.`,
      });
    } catch (error) {
      toast.error(`Could not print ${what}`, { description: message(error) });
    } finally {
      setPrinting((n) => n - 1);
    }
  };

  const print = async () => {
    if (!canPrint) return;
    await send(job, image, NAMES[workflow]);
  };

  /** Faxes the job in the form; rejects, after saying why, if it did
   * not go. */
  const sendFax = (to: string) => {
    const what = NAMES[workflow];
    return tryTo(`Could not fax ${what}`, async () => {
      const sent = await api.sendFax(to, job, image);
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

  const saveSchedule = ({ id, ...spec }: Draft) =>
    attempt("Could not save the schedule", async () => {
      if (id) await api.replaceSchedule(id, spec);
      else await api.createSchedule(spec);
      await refresh();
      setDraft(null);
    });

  /** Points every schedule at one printer, and says which stayed put. */
  const moveSchedules = (printer: string) =>
    attempt("Could not move the schedules", async () => {
      const { asked, refused } = await repoint(
        api,
        (s) => s.printer !== printer,
        printer,
      );
      if (refused.length > 0) {
        toast.error(
          `Could not move ${refused.length} of ${asked} to ${printer}`,
          { description: refused.map(refusal).join("; ") },
        );
      } else {
        toast.success(`Moved ${asked} to ${printer}`);
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
            onDelete={(name) =>
              attempt(
                "Could not delete the profile",
                thenRefresh(() => api.deletePrinter(name)),
              )
            }
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
          <PrintOptions profile={profile} onChange={changeProfile} />
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[var(--col)_minmax(0,1fr)]">
        <section className="flex flex-col gap-4 border-r p-4">
          <Tabs
            value={workflow}
            onValueChange={(value) => {
              if (isKind(value)) setWorkflow(value);
            }}
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

          {/* A form hands back what it holds without its kind. */}
          {workflow === "task-card" ? (
            <TaskCardForm
              card={jobs["task-card"]}
              onChange={(card) => put({ kind: "task-card", ...card })}
              onSubmit={print}
            />
          ) : workflow === "text" ? (
            <TextForm
              text={jobs.text}
              kind={kind}
              onChange={(text) => put({ kind: "text", ...text })}
              onSubmit={print}
            />
          ) : workflow === "note" ? (
            <NoteForm
              note={jobs.note}
              onChange={(note) => put({ kind: "note", ...note })}
            />
          ) : workflow === "qr" ? (
            <QrForm
              code={jobs.qr}
              onChange={(code) => put({ kind: "qr", ...code })}
              onSubmit={print}
            />
          ) : workflow === "test-page" ? (
            <TestPageForm
              page={jobs["test-page"]}
              kind={kind}
              onChange={(page) => put({ kind: "test-page", ...page })}
            />
          ) : (
            <PictureForm
              picture={jobs.picture}
              file={file}
              thermal={kind === "thermal"}
              twoColor={twoColor}
              onChange={(picture) => put({ kind: "picture", ...picture })}
              onFile={setFile}
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
                {printing > 0 ? "Printing…" : "Print"}
              </Button>
              <Button
                variant="outline"
                disabled={
                  !ready || !schedules || !profile || job.kind === "picture"
                }
                title={
                  job.kind === "picture"
                    ? "Pictures cannot be scheduled."
                    : undefined
                }
                onClick={() => {
                  if (profile && job.kind !== "picture") {
                    setDraft(newSchedule(job, profile.name));
                  }
                }}
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
                  tryTo(
                    "Could not save the contact",
                    thenRefresh(() => api.putContact(number, name)),
                  )
                }
                onRemoveContact={(contact) =>
                  attempt(
                    "Could not remove the contact",
                    thenRefresh(() => api.deleteContact(contact.number)),
                  )
                }
              />
              {!profile && (
                <span className="text-sm text-destructive">No printer.</span>
              )}
              <Toggle
                id="cut"
                className="ml-auto w-auto"
                checked={profile?.cut ?? true}
                disabled={!profile}
                onCheckedChange={(cut) =>
                  profile && changeProfile({ ...profile, cut })
                }
              >
                Auto cut
              </Toggle>
            </div>
          </div>
        </section>

        {profile ? (
          <JobPreview
            api={api}
            printer={profile}
            kind={workflow}
            job={job}
            image={image}
          />
        ) : (
          <PreviewPane>
            <p className="text-sm text-muted-foreground">
              {reachable === "offline"
                ? `${server.url} is not answering.`
                : "Add a printer under Profile actions to begin."}
            </p>
          </PreviewPane>
        )}
      </div>

      {schedules && (
        <SchedulesDrawer
          open={drawer}
          onOpenChange={setDrawer}
          schedules={schedules}
          profiles={profiles}
          onEnable={({ id, ...spec }, enabled) =>
            attempt(
              "Could not change the schedule",
              thenRefresh(() => api.replaceSchedule(id, { ...spec, enabled })),
            )
          }
          onEdit={setDraft}
          onLoad={(schedule) => {
            load(schedule.job);
            setDrawer(false);
          }}
          onDelete={({ id }) =>
            attempt(
              "Could not delete the schedule",
              thenRefresh(() => api.deleteSchedule(id)),
            )
          }
          onMoveAll={moveSchedules}
          clock={
            remote ? `the clock at ${server.url}` : "this computer's clock"
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
            attempt("Could not activate the line", thenRefresh(api.activateFax))
          }
          onReplace={() =>
            attempt(
              "Could not get a new number",
              thenRefresh(api.replaceFaxLine),
            )
          }
          onShowCode={(number) => {
            load({ kind: "qr", ...DEFAULT_QR, data: number, caption: number });
            setFaxDrawer(false);
          }}
          onSettings={(changes) =>
            attempt(
              "Could not save the fax settings",
              thenRefresh(() => api.putFax(changes)),
            )
          }
          onAddRelay={(url) =>
            tryTo(
              "Could not add the relay",
              thenRefresh(async () => {
                const relay = await api.addRelay(url);
                toast.success(`Added ${relay.name}`, {
                  description: relay.url,
                });
              }),
            )
          }
          onRemoveRelay={(relay) =>
            attempt(
              "Could not remove the relay",
              thenRefresh(() => api.removeRelay(relay.name)),
            )
          }
        />
      )}

      <ScheduleDialog
        draft={draft}
        profiles={profiles}
        runsWhile={remote ? `${server.url} is running` : "the app is open"}
        onClose={() => setDraft(null)}
        onSave={saveSchedule}
      />
    </main>
  );
}
