import { useEffect, useState, type DependencyList } from "react";

/**
 * The latest result of `compute`, or null until the first one lands. A
 * result that `deps` have outdated is dropped.
 */
export function useAsync<T>(
  compute: () => Promise<T>,
  deps: DependencyList,
): T | null {
  const [value, setValue] = useState<T | null>(null);
  useEffect(() => {
    let cancelled = false;
    compute()
      .then((result) => {
        if (!cancelled) setValue(result);
      })
      .catch(console.error);
    return () => {
      cancelled = true;
    };
    // The caller's inputs; `compute` reads nothing else.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return value;
}
