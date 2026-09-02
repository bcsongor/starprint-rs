import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { whoami } from "@/lib/linear";
import type { Linear } from "@/lib/settings";

interface Props {
  linear: Linear | null;
  onChange: (linear: Linear) => void;
}

/** The Linear mark, from simple-icons (CC0), in the button's own colour. */
function LinearLogo({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden
      className={className}
    >
      <path d="M2.886 4.18A11.982 11.982 0 0 1 11.99 0C18.624 0 24 5.376 24 12.009c0 3.64-1.62 6.903-4.18 9.105L2.887 4.18ZM1.817 5.626l16.556 16.556c-.524.33-1.075.62-1.65.866L.951 7.277c.247-.575.537-1.126.866-1.65ZM.322 9.163l14.515 14.515c-.71.172-1.443.282-2.195.322L0 11.358a12 12 0 0 1 .322-2.195Zm-.17 4.862 9.823 9.824a12.02 12.02 0 0 1-9.824-9.824Z" />
    </svg>
  );
}

/**
 * Connects a Linear account with a personal API key, then offers the
 * auto-print toggle. Disconnecting keeps the key, so the dialog comes
 * back filled in. The printing itself is `useAutoPrint`.
 */
export function LinearBar({ linear, onChange }: Props) {
  const [open, setOpen] = useState(false);

  return (
    <div className="flex items-center gap-2 border-t pt-4">
      {linear?.user ? (
        <>
          <LinearLogo className="size-4 text-muted-foreground" />
          <Field orientation="horizontal" className="w-auto">
            <Switch
              id="auto-print"
              checked={linear.autoPrint}
              onCheckedChange={(autoPrint) =>
                onChange({ ...linear, autoPrint })
              }
            />
            <FieldLabel
              htmlFor="auto-print"
              className="text-sm tracking-normal normal-case text-foreground"
            >
              Auto-print my new issues
            </FieldLabel>
          </Field>
          <Button
            variant="ghost"
            size="sm"
            className="ml-auto text-muted-foreground"
            title={`Connected as ${linear.user}`}
            onClick={() =>
              onChange({ ...linear, user: null, autoPrint: false })
            }
          >
            Disconnect
          </Button>
        </>
      ) : (
        <Button onClick={() => setOpen(true)}>
          <LinearLogo />
          Connect to Linear
        </Button>
      )}

      <Dialog open={open} onOpenChange={setOpen}>
        {/* Mounted per opening so the draft starts from the stored key. */}
        {open && (
          <ConnectForm
            initialKey={linear?.apiKey ?? ""}
            onCancel={() => setOpen(false)}
            onConnect={(apiKey, user) => {
              onChange({ apiKey, user, autoPrint: false });
              setOpen(false);
            }}
          />
        )}
      </Dialog>
    </div>
  );
}

function ConnectForm({
  initialKey,
  onCancel,
  onConnect,
}: {
  initialKey: string;
  onCancel: () => void;
  onConnect: (apiKey: string, user: string) => void;
}) {
  const [apiKey, setApiKey] = useState(initialKey);
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const key = apiKey.trim();

  const connect = async () => {
    if (!key || checking) return;
    setChecking(true);
    setError(null);
    try {
      const user = await whoami(key);
      toast.success(`Connected to Linear as ${user}`);
      onConnect(key, user);
    } catch (error) {
      setError(String(error));
    } finally {
      setChecking(false);
    }
  };

  return (
    <DialogContent className="gap-3 sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Connect to Linear</DialogTitle>
        <DialogDescription>
          Paste a personal API key. Linear issues one under Settings, Security
          &amp; access, Personal API keys. Read access is enough.
        </DialogDescription>
      </DialogHeader>

      <Field data-invalid={error !== null}>
        <FieldLabel htmlFor="linear-key">API key</FieldLabel>
        <Input
          id="linear-key"
          type="password"
          value={apiKey}
          placeholder="lin_api_…"
          autoFocus
          spellCheck={false}
          autoComplete="off"
          aria-invalid={error !== null}
          onChange={(e) => setApiKey(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              connect();
            }
          }}
        />
        {error && <p className="text-sm text-destructive">{error}</p>}
      </Field>

      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button disabled={!key || checking} onClick={connect}>
          {checking ? "Connecting…" : "Connect"}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
