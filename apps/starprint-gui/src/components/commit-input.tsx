import { useState } from "react";
import { Input } from "@/components/ui/input";

/**
 * An input whose value lands on Enter or blur, not on every keystroke.
 * Addresses and tokens are not for the webview's autofill to remember.
 */
export function CommitInput({
  value,
  onCommit,
  ...props
}: {
  value: string;
  onCommit: (value: string) => void;
} & Omit<
  React.ComponentProps<typeof Input>,
  "value" | "onChange" | "onBlur" | "onKeyDown"
>) {
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft !== null && draft !== value) onCommit(draft);
    setDraft(null);
  };
  return (
    <Input
      autoComplete="off"
      {...props}
      value={draft ?? value}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          commit();
        }
      }}
    />
  );
}
