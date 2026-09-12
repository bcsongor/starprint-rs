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
import { Textarea } from "@/components/ui/textarea";
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
  // Kept as typed so the field can be cleared while editing.
  const [port, setPort] = useState(String(profile.port));
  const set = (changes: Partial<Pick<Profile, "name" | "host" | "notes">>) =>
    setDraft({ ...draft, ...changes });
  /** A kind brings the fields it has and sheds the rest. */
  const setKind = (kind: PrinterKind) => {
    const { name, host, port, cut, notes } = draft;
    const shared = { name, host, port, cut, notes };
    setDraft(
      kind === "thermal"
        ? { ...shared, kind, paper: 80, density: 3, speed: "slow" }
        : { ...shared, kind },
    );
  };
  const kind = KINDS.find((k) => k.value === draft.kind) ?? KINDS[0];
  const name = draft.name.trim();
  // `PUT` replaces by name, so another profile's name would overwrite it.
  const collides = taken.includes(name);
  const portNumber = Number(port);
  const portValid =
    /^\d+$/.test(port) && portNumber >= 1 && portNumber <= 65535;
  const valid =
    name !== "" && !collides && draft.host.trim() !== "" && portValid;

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
            autoComplete="off"
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
              className="font-mono"
              value={draft.host}
              placeholder="192.168.1.60"
              spellCheck={false}
              autoComplete="off"
              onChange={(e) => set({ host: e.target.value })}
            />
          </Field>
          <Field data-invalid={!portValid}>
            <FieldLabel htmlFor="port">Port</FieldLabel>
            <Input
              id="port"
              className="text-center font-mono"
              inputMode="numeric"
              value={port}
              aria-invalid={!portValid}
              onChange={(e) => setPort(e.target.value)}
            />
          </Field>
        </div>

        <Field>
          <FieldLabel htmlFor="notes">Notes</FieldLabel>
          <Textarea
            id="notes"
            className="max-h-40 text-sm"
            value={draft.notes}
            placeholder="Firmware, interface card, where it sits"
            autoComplete="off"
            onChange={(e) => set({ notes: e.target.value })}
          />
        </Field>
      </FieldGroup>

      <DialogFooter>
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button
          disabled={!valid}
          onClick={() =>
            onSave({
              ...draft,
              name,
              host: draft.host.trim(),
              port: portNumber,
              notes: draft.notes.trim(),
            })
          }
        >
          Save
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}
