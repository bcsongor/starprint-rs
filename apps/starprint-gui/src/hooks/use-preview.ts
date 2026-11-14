import { useEffect, useState } from "react";
import type { Client, Preview, Profile } from "@/lib/api";
import type { Jobs, Kind, Previews } from "@/lib/job";
import { message } from "@/lib/message";

/** Coalesces slider drags; the preview is never more than one render behind. */
const DEBOUNCE_MS = 30;

/** One request at a time; a job outdated while waiting is never sent. */
let chain: Promise<void> = Promise.resolve();

export interface Previewed<K extends Kind> {
  /** One for each job, in order. */
  previews: Previews[K][] | null;
  error: string | null;
}

/** The server's last answer, and the kind of job it was about. */
interface Answer {
  kind: Kind;
  previews: Preview[] | null;
  error: string | null;
}

function isOf<K extends Kind>(
  preview: Preview,
  kind: K,
): preview is Previews[K] {
  return preview.kind === kind;
}

/**
 * What the server says `jobs`, of `kind`, will look like on `printer`,
 * one preview each. Empty when there is nothing to show; an answer that
 * a change has outdated is dropped. The previous previews stay up while
 * their replacements load, unless they were of jobs of another kind.
 */
export function usePreviews<K extends Kind>(
  api: Client,
  printer: Profile,
  kind: K,
  jobs: Jobs[K][] | null,
  image: File | null,
): Previewed<K> {
  const [answer, setAnswer] = useState<Answer | null>(null);
  // Both are rebuilt each render, the profile by every poll; compare by
  // value. The profile's settings shape the preview as the jobs do.
  const key = JSON.stringify([printer, jobs]);
  useEffect(() => {
    if (!jobs) {
      setAnswer(null);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      chain = chain.then(async () => {
        if (cancelled) return;
        try {
          const previews = await Promise.all(
            jobs.map((job) => api.preview(printer.name, job, image)),
          );
          if (!cancelled) setAnswer({ kind, previews, error: null });
        } catch (error) {
          if (!cancelled) {
            setAnswer({ kind, previews: null, error: message(error) });
          }
        }
      });
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // `printer` and `jobs` by value; nothing else is read.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [api, key, image]);

  if (answer?.kind !== kind) return { previews: null, error: null };
  const { previews, error } = answer;
  return {
    previews:
      previews?.filter((preview): preview is Previews[K] =>
        isOf(preview, kind),
      ) ?? null,
    error,
  };
}
