import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { PrinterKind, Profile } from "@/lib/api";

const KINDS: { value: PrinterKind; label: string; models: string }[] = [
  {
    value: "thermal",
    label: "Thermal",
    models: "TSP650II, TSP700II, TSP800II",
  },
  { value: "impact", label: "Impact", models: "SP712, SP742, SP717, SP747" },
];

interface Props {
  /** The profile to edit, or null when closed. */
  profile: Profile | null;
  /** The other profiles' names, which this one cannot take. */
  taken: string[];
  onClose: () => void;
  onSave: (profile: Profile) => void;
}

/** Edits a copy of the profile; nothing is applied until Save. */
export function ProfileDialog({ profile, taken, onClose, onSave }: Props) {
  return (
    <Dialog
      open={profile !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      {/* Mounted per opening so the draft starts from the profile. */}
      {profile && (
        <ProfileForm
          profile={profile}
          taken={taken}
          onCancel={onClose}
          onSave={onSave}
        />
      )}
    </Dialog>
  );
}

function ProfileForm({
  profile,
  taken,
  onCancel,
  onSave,
}: {
  profile: Profile;
  taken: string[];
  onCancel: () => void;
  onSave: (profile: Profile) => void;
}) {
  const [draft, setDraft] = useState(profile);
  const set = (changes: Partial<Pick<Profile, "name" | "host" | "port">>) =>
    setDraft({ ...draft, ...changes });
  /** A kind brings the fields it has and sheds the rest. */
  const setKind = (kind: PrinterKind) => {
    const { name, host, port, cut } = draft;
    setDraft(
      kind === "thermal"
        ? { name, host, port, cut, kind, paper: 80, density: 3, speed: "slow" }
        : { name, host, port, cut, kind },
    );
  };
  const kind = KINDS.find((k) => k.value === draft.kind) ?? KINDS[0];
  const name = draft.name.trim();
  // `PUT` replaces by name, so another profile's name would overwrite it.
  const collides = taken.includes(name);
  const valid = name !== "" && !collides && draft.host.trim() !== "";

  return (
    <DialogContent className="gap-3 sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Profile</DialogTitle>
      </DialogHeader>

      <FieldGroup className="gap-3">
        <Field data-invalid={collides}>
          <FieldLabel htmlFor="profile-name">Name</FieldLabel>
          <Input
            id="profile-name"
            value={draft.name}
            autoFocus
            aria-invalid={collides}
            onChange={(e) => set({ name: e.target.value })}
          />
          {collides && (
            <p className="text-sm text-destructive">
              There is already a profile called {name}.
            </p>
          )}
        </Field>

        <Field>
          <FieldLabel htmlFor="kind">Printer</FieldLabel>
          <Select
            modal={false}
            value={draft.kind}
            onValueChange={(value) => setKind(value as PrinterKind)}
          >
            <SelectTrigger id="kind" className="w-full">
              <SelectValue>
                {kind.label}
                <span className="text-muted-foreground"> · {kind.models}</span>
              </SelectValue>
            </SelectTrigger>
            <SelectContent alignItemWithTrigger={false} align="start">
              {KINDS.map((k) => (
                <SelectItem key={k.value} value={k.value}>
                  {k.label}
                  <span className="text-muted-foreground"> · {k.models}</span>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>

        <div className="grid grid-cols-[1fr_5rem] gap-3">
          <Field>
            <FieldLabel htmlFor="host">Host</FieldLabel>
            <Input
              id="host"
              value={draft.host}
              placeholder="192.168.1.60"
              spellCheck={false}
              autoComplete="off"
              onChange={(e) => set({ host: e.target.value })}
            />
          </Field>
          <Field>
            <FieldLabel htmlFor="port">Port</FieldLabel>
            <Input
              id="port"
              type="number"
              min={1}
              max={65535}
              value={draft.port}
              onChange={(e) => set({ port: Number(e.target.value) || 9100 })}
            />
          </Field>
        </div>
      </FieldGroup>

      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          disabled={!valid}
          onClick={() =>
            onSave({ ...draft, name, host: draft.host.trim() })
          }
        >
          Save
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
