import { useEffect, useState } from "react";
import type { Client, Job, Preview } from "@/lib/api";

/** Coalesces slider drags; the preview is never more than one render behind. */
const DEBOUNCE_MS = 30;

/** One request at a time; a job outdated while waiting is never sent. */
let chain: Promise<void> = Promise.resolve();

export interface Previewed {
  preview: Preview | null;
  error: string | null;
}

const EMPTY: Previewed = { preview: null, error: null };

/**
 * What the server says `job` will look like on `printer`. Empty when
 * there is no job to show; an answer that a change has outdated is
 * dropped. The previous preview stays up while its replacement loads.
 */
export function usePreview(
  api: Client,
  printer: string | null,
  job: Job | null,
  image: File | null,
): Previewed {
  const [state, setState] = useState<Previewed>(EMPTY);
  // Jobs are rebuilt each render; compare by value.
  const key = JSON.stringify(job);
  useEffect(() => {
    if (!printer || !job) {
      setState(EMPTY);
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      chain = chain.then(async () => {
        if (cancelled) return;
        try {
          const preview = await api.preview(printer, job, image);
          if (!cancelled) setState({ preview, error: null });
        } catch (error) {
          if (!cancelled) setState({ preview: null, error: String(error) });
        }
      });
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // `job` by value; nothing else is read.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [api, printer, key, image]);
  return state;
}
