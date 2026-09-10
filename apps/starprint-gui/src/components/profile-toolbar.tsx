import { useState } from "react";
import { EllipsisIcon } from "lucide-react";
import { ConnectionDot, DOTTED } from "@/components/connection-dot";
import { ProfileDialog } from "@/components/profile-dialog";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { statusLabel, type Status } from "@/hooks/use-statuses";
import { byName, type Profile, type ProfileSpec } from "@/lib/api";

/** A name that does not collide with the existing profiles. */
function uniqueName(base: string, profiles: Profile[]): string {
  const taken = new Set(profiles.map((p) => p.name));
  if (!taken.has(base)) return base;
  for (let n = 2; ; n++) {
    const candidate = `${base} ${n}`;
    if (!taken.has(candidate)) return candidate;
  }
}

/** What a new profile starts from when there is none to copy. */
const FRESH: ProfileSpec = {
  host: "",
  port: 9100,
  kind: "thermal",
  cut: true,
  paper: 80,
  density: 3,
  speed: "slow",
};

interface Props {
  profiles: Profile[];
  /** The one the picker shows; undefined when there are none. */
  profile: Profile | undefined;
  status: (name: string) => Status;
  onSelect: (name: string) => void;
  /** Creates or replaces `profile`, then removes `replacing` if it was renamed. */
  onSave: (profile: Profile, replacing: string | null) => void;
  onDelete: (name: string) => void;
}

/** Profile picker plus its ⋯ menu; the settings live in a dialog. */
export function ProfileToolbar({
  profiles,
  profile,
  status,
  onSelect,
  onSave,
  onDelete,
}: Props) {
  /** The profile in the dialog, and the name it replaces on save. */
  const [editing, setEditing] = useState<{
    profile: Profile;
    replacing: string | null;
  } | null>(null);
  const listed = byName(profiles);

  return (
    <Field className="min-w-0 flex-1">
      <FieldLabel htmlFor="profile">Profile</FieldLabel>
      <div className="flex items-center gap-2">
        <Select
          modal={false}
          disabled={!profile}
          value={profile?.name ?? null}
          onValueChange={(name) => {
            if (name) onSelect(name);
          }}
        >
          <SelectTrigger id="profile" className="min-w-0 flex-1">
            <SelectValue placeholder="No printers">
              {profile && (
                <>
                  <span className={DOTTED}>
                    <ConnectionDot
                      status={status(profile.name)}
                      label={statusLabel(profile, status(profile.name))}
                    />
                    {profile.name}
                  </span>
                  <span className="truncate text-muted-foreground">
                    {" "}
                    · {profile.host}
                  </span>
                </>
              )}
            </SelectValue>
          </SelectTrigger>
          <SelectContent
            alignItemWithTrigger={false}
            align="start"
            className="w-max max-w-(--available-width) min-w-(--anchor-width)"
          >
            {listed.map((p) => (
              <SelectItem key={p.name} value={p.name}>
                <span className={DOTTED}>
                  <ConnectionDot
                    status={status(p.name)}
                    label={statusLabel(p, status(p.name))}
                  />
                  {p.name}
                </span>
                <span className="text-muted-foreground"> · {p.host}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <DropdownMenu modal={false}>
          <DropdownMenuTrigger
            render={
              <Button
                variant="outline"
                size="icon"
                aria-label="Profile actions"
              />
            }
          >
            <EllipsisIcon />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            <DropdownMenuItem
              disabled={!profile}
              onClick={() =>
                profile && setEditing({ profile, replacing: profile.name })
              }
            >
              Edit…
            </DropdownMenuItem>
            <DropdownMenuItem
              disabled={!profile}
              onClick={() =>
                profile &&
                onSave(
                  {
                    ...profile,
                    name: uniqueName(`${profile.name} copy`, profiles),
                  },
                  null,
                )
              }
            >
              Duplicate
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() =>
                setEditing({
                  profile: {
                    ...(profile ?? FRESH),
                    name: uniqueName("New profile", profiles),
                    host: "",
                  },
                  replacing: null,
                })
              }
            >
              New profile…
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              disabled={!profile}
              onClick={() => profile && onDelete(profile.name)}
            >
              Delete
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <ProfileDialog
          profile={editing?.profile ?? null}
          taken={profiles
            .map((p) => p.name)
            .filter((name) => name !== editing?.replacing)}
          onClose={() => setEditing(null)}
          onSave={(next) => {
            const replacing = editing?.replacing ?? null;
            setEditing(null);
            onSave(next, replacing === next.name ? null : replacing);
          }}
        />
      </div>
    </Field>
  );
}
