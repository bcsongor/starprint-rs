import { useState } from "react";
import { EllipsisIcon } from "lucide-react";
import { ConnectionDot, DOTTED } from "@/components/connection-dot";
import { statusLabel, useProbes } from "@/hooks/use-probes";
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
import {
  activeProfile,
  newId,
  type Profile,
  type Profiles,
} from "@/lib/settings";

/** A name that does not collide with the existing profiles. */
function uniqueName(base: string, profiles: Profile[]): string {
  const taken = new Set(profiles.map((p) => p.name));
  if (!taken.has(base)) return base;
  for (let n = 2; ; n++) {
    const candidate = `${base} ${n}`;
    if (!taken.has(candidate)) return candidate;
  }
}

interface Props {
  state: Profiles;
  onChange: (state: Profiles) => void;
  /** Suspends the connection probe while a job is printing. */
  printing: boolean;
}

/** Profile picker plus its ⋯ menu; the settings live in a dialog. */
export function ProfileToolbar({ state, onChange, printing }: Props) {
  const [editing, setEditing] = useState(false);
  const profile = activeProfile(state);
  const statuses = useProbes(state.profiles, state.activeId, printing);

  const add = (next: Profile) =>
    onChange({ profiles: [...state.profiles, next], activeId: next.id });

  return (
    <Field className="w-80">
      <FieldLabel htmlFor="profile">Profile</FieldLabel>
      <div className="flex items-center gap-2">
        <Select
          value={profile.id}
          onValueChange={(activeId) => {
            if (activeId) onChange({ ...state, activeId });
          }}
        >
          <SelectTrigger id="profile" className="min-w-0 flex-1">
            <SelectValue>
              <span className={DOTTED}>
                <ConnectionDot
                  status={statuses[profile.id] ?? "checking"}
                  label={statusLabel(
                    profile,
                    statuses[profile.id] ?? "checking",
                  )}
                />
                {profile.name}
              </span>
              <span className="text-muted-foreground">
                {" "}
                · {profile.host || "no host"}
              </span>
            </SelectValue>
          </SelectTrigger>
          {/* Padded, so the dot does not sit on the edge. */}
          <SelectContent
            alignItemWithTrigger={false}
            align="start"
            className="p-1"
          >
            {state.profiles.map((p) => (
              <SelectItem key={p.id} value={p.id}>
                <span className={DOTTED}>
                  <ConnectionDot
                    status={statuses[p.id] ?? "checking"}
                    label={statusLabel(p, statuses[p.id] ?? "checking")}
                  />
                  {p.name}
                </span>
                <span className="text-muted-foreground">
                  {" "}
                  · {p.host || "no host"}
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <DropdownMenu>
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
            <DropdownMenuItem onClick={() => setEditing(true)}>
              Edit…
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() =>
                add({
                  ...profile,
                  id: newId(),
                  name: uniqueName(`${profile.name} copy`, state.profiles),
                })
              }
            >
              Duplicate
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                add({
                  ...profile,
                  id: newId(),
                  name: uniqueName("New profile", state.profiles),
                  host: "",
                });
                setEditing(true);
              }}
            >
              New profile…
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              disabled={state.profiles.length <= 1}
              onClick={() => {
                const profiles = state.profiles.filter(
                  (p) => p.id !== profile.id,
                );
                onChange({ profiles, activeId: profiles[0].id });
              }}
            >
              Delete
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <ProfileDialog
          profile={profile}
          open={editing}
          onOpenChange={setEditing}
          onSave={(next) =>
            onChange({
              ...state,
              profiles: state.profiles.map((p) =>
                p.id === next.id ? next : p,
              ),
            })
          }
        />
      </div>
    </Field>
  );
}
