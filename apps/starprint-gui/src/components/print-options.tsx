import { OptionSelect, type Option } from "@/components/option-select";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { TWO_COLOR_DENSITY, type Profile, type Speed } from "@/lib/api";
import { roll } from "@/lib/paper";

const PAPERS: Option<string>[] = [
  { value: "80", label: "80 mm", hint: "576 dots" },
  { value: "112", label: "112 mm", hint: "832 dots" },
];

const DENSITIES: Option<string>[] = [
  { value: String(TWO_COLOR_DENSITY), label: "+4", hint: "two-colour" },
  { value: "3", label: "+3", hint: "darkest" },
  { value: "2", label: "+2" },
  { value: "1", label: "+1" },
  { value: "0", label: "0", hint: "default" },
  { value: "-1", label: "-1" },
  { value: "-2", label: "-2" },
  { value: "-3", label: "-3", hint: "lightest" },
];

const SPEEDS: Option<Speed>[] = [
  { value: "slow", label: "Slow", hint: "best quality" },
  { value: "medium", label: "Medium", hint: "" },
  { value: "high", label: "High", hint: "default" },
];

interface Props {
  /** Undefined while there is no profile; the controls show disabled. */
  profile: Profile | undefined;
  onChange: (profile: Profile) => void;
}

/** Paper width, with density and speed controls for thermal printers. */
export function PrintOptions({ profile, onChange }: Props) {
  const thermal = profile?.kind === "thermal";
  const twoColor = thermal && profile.density === TWO_COLOR_DENSITY;

  return (
    <>
      <Field data-disabled={!thermal}>
        <FieldLabel htmlFor="paper">Paper</FieldLabel>
        {thermal ? (
          <OptionSelect
            id="paper"
            value={String(profile.paper)}
            options={PAPERS}
            labelClassName="w-14"
            onChange={(paper) =>
              onChange({ ...profile, paper: paper === "112" ? 112 : 80 })
            }
          />
        ) : (
          <Input
            id="paper"
            value={profile ? `${roll(profile).paperMm} mm` : ""}
            disabled
          />
        )}
      </Field>

      {thermal && (
        <>
          <Field>
            <FieldLabel htmlFor="density">Density</FieldLabel>
            <OptionSelect
              id="density"
              value={String(profile.density)}
              options={DENSITIES}
              labelClassName="w-6 text-right tabular-nums"
              onChange={(density) =>
                onChange({ ...profile, density: Number(density) })
              }
            />
          </Field>

          <Field
            data-disabled={twoColor}
            title={twoColor ? "Two-colour mode has one speed." : undefined}
          >
            <FieldLabel htmlFor="speed">Speed</FieldLabel>
            <OptionSelect
              id="speed"
              value={profile.speed}
              options={SPEEDS}
              disabled={twoColor}
              labelClassName="w-14"
              onChange={(speed) => onChange({ ...profile, speed })}
            />
          </Field>
        </>
      )}
    </>
  );
}
