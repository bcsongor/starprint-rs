import type { ReactNode } from "react";
import { CardPreview } from "@/components/card-preview";
import { NotePreview } from "@/components/note-preview";
import { PicturePreview } from "@/components/picture-preview";
import { QrPreview } from "@/components/qr-preview";
import { TestPagePreview } from "@/components/test-page-preview";
import { TextPreview } from "@/components/text-preview";
import { usePreview, type Previewed } from "@/hooks/use-preview";
import type { Client, Profile } from "@/lib/api";
import { isPreviewable, type Jobs, type Kind } from "@/lib/job";

/** Each kind's pane, given the job in the form and the server's
 * preview of it. */
type Panes = {
  [K in Kind]: (
    shown: Previewed<K> & { job: Jobs[K]; printer: Profile },
  ) => ReactNode;
};

const PANES: Panes = {
  "task-card": ({ preview, error, printer }) => (
    <CardPreview layout={preview} error={error} printer={printer} />
  ),
  text: ({ preview, error, printer }) => (
    <TextPreview layout={preview} error={error} printer={printer} />
  ),
  note: ({ job, preview, error, printer }) => (
    <NotePreview note={job} preview={preview} error={error} printer={printer} />
  ),
  qr: ({ preview, error, printer }) => (
    <QrPreview preview={preview} error={error} printer={printer} />
  ),
  "test-page": ({ preview, error }) => (
    <TestPagePreview sections={preview?.sections ?? []} error={error} />
  ),
  picture: ({ preview, error, printer }) => (
    <PicturePreview
      image={preview?.image ?? null}
      error={error}
      printer={printer}
    />
  ),
};

interface Props<K extends Kind> {
  api: Client;
  printer: Profile;
  /** The job's own kind, apart from it so the pane can be picked by type. */
  kind: K;
  job: Jobs[K];
  /** The picture a picture job is of. */
  image: File | null;
}

/** The server's preview of the job in the form, in its kind's pane. */
export function JobPreview<K extends Kind>({
  api,
  printer,
  kind,
  job,
  image,
}: Props<K>) {
  const previewed = usePreview(
    api,
    printer,
    kind,
    isPreviewable(job, image) ? job : null,
    image,
  );
  return PANES[kind]({ ...previewed, job, printer });
}
