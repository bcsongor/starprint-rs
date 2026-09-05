import { useEffect, useState, type DependencyList } from "react";

/** Coalesces slider drags; the preview is never more than one render behind. */
const DEBOUNCE_MS = 30;

/** One render at a time across every preview; a stale request is skipped. */
let chain: Promise<void> = Promise.resolve();

export interface Preview<T> {
  value: T | null;
  /** An object URL of the PNG, revoked when the next one replaces it. */
  url: string | null;
  error: string | null;
}

const EMPTY: Preview<never> = { value: null, url: null, error: null };

/**
 * A PNG the Rust side draws, as an object URL, with whatever else
 * `render` returns beside it. Empty when `render` resolves to null, and
 * a render that `deps` have outdated is dropped. The previous image
 * stays visible while its replacement renders.
 * `onReady` clears readiness on input changes and unmount.
 */
export function usePreview<T>(
  render: () => Promise<{ value: T; png: ArrayBuffer } | null>,
  deps: DependencyList,
  onReady?: (ready: boolean) => void,
): Preview<T> {
  const [preview, setPreview] = useState<Preview<T>>(EMPTY);
  useEffect(() => {
    const url = preview.url;
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  }, [preview.url]);
  useEffect(() => {
    let cancelled = false;
    onReady?.(false);
    const timer = setTimeout(() => {
      chain = chain.then(async () => {
        if (cancelled) return;
        try {
          const rendered = await render();
          if (cancelled) return;
          if (!rendered) {
            setPreview(EMPTY);
            return;
          }
          const url = URL.createObjectURL(
            new Blob([rendered.png], { type: "image/png" }),
          );
          setPreview({ value: rendered.value, url, error: null });
          onReady?.(true);
        } catch (error) {
          if (cancelled) return;
          setPreview({ ...EMPTY, error: String(error) });
        }
      });
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      onReady?.(false);
    };
    // The caller's render inputs; `onReady` is a stable state setter.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return preview;
}
