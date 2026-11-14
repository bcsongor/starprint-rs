import { useEffect, useState } from "react";
import type { Client, Preview, Profile } from "@/lib/api";
import type { Jobs, Kind, Previews } from "@/lib/job";
import { message } from "@/lib/message";

/** Coalesces slider drags; the preview is never more than one render behind. */
const DEBOUNCE_MS = 30;

/** One request at a time; a job outdated while waiting is never sent. */
let chain: Promise<void> = Promise.resolve();

export interface Previewed<K extends Kind> {
  preview: Previews[K] | null;
  error: string | null;
}

/** The server's last answer, and the kind of job it was about. */
interface Answer {
  kind: Kind;
  preview: Preview | null;
  error: string | null;
}

function isOf<K extends Kind>(
  preview: Preview,
  kind: K,
): preview is Previews[K] {
  return preview.kind === kind;
}

/**
 * What the server says `job`, of `kind`, will look like on `printer`.
 * Empty when there is no job to show; an answer that a change has
 * outdated is dropped. The previous preview stays up while its
 * replacement loads, unless it was of a job of another kind.
 */
export function usePreview<K extends Kind>(
  api: Client,
  printer: Profile,
  kind: K,
  job: Jobs[K] | null,
  image: File | null,
): Previewed<K> {
  const [answer, setAnswer] = useState<Answer | null>(null);
  // Both are rebuilt each render, the profile by every poll; compare by
  // value. The profile's settings shape the preview as the job does.
  const key = JSON.stringify([printer, job]);
  useEffect(() => {
    if (!job) {
      setAnswer(null);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      chain = chain.then(async () => {
        if (cancelled) return;
        try {
          const preview = await api.preview(printer.name, job, image);
          if (!cancelled) setAnswer({ kind, preview, error: null });
        } catch (error) {
          if (!cancelled) {
            setAnswer({ kind, preview: null, error: message(error) });
          }
        }
      });
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // `printer` and `job` by value; nothing else is read.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [api, key, image]);

  if (answer?.kind !== kind) return { preview: null, error: null };
  const { preview, error } = answer;
  return { preview: preview && isOf(preview, kind) ? preview : null, error };
}
