import { load } from "@tauri-apps/plugin-store";
import type { Printer } from "./api";

const FILE = "settings.json";
const KEY = "profiles";
/** Key used before profiles existed; migrated on first load. */
const LEGACY_KEY = "printer";

/** A named printer configuration. */
export interface Profile extends Printer {
  id: string;
  name: string;
}

export interface Profiles {
  profiles: Profile[];
  activeId: string;
}

const THERMAL: Printer = {
  kind: "thermal",
  host: "",
  port: 9100,
  density: 3,
  speed: "slow",
  paper: "80",
  cut: true,
};

const SEED: Profile[] = [
  { ...THERMAL, id: "tsp700ii", name: "TSP700II", host: "" },
  { ...THERMAL, id: "tsp800ii", name: "TSP800II", host: "192.168.1.180" },
  {
    ...THERMAL,
    id: "sp743",
    name: "SP743",
    kind: "impact",
    host: "192.168.1.141",
  },
];

export const DEFAULT_PROFILES: Profiles = {
  profiles: SEED,
  activeId: "tsp800ii",
};

export function newId(): string {
  return crypto.randomUUID();
}

/** The printer settings of a profile, without its identity. */
export function toPrinter(profile: Profile): Printer {
  const { id: _id, name: _name, ...printer } = profile;
  return printer;
}

export function activeProfile(state: Profiles): Profile {
  return (
    state.profiles.find((p) => p.id === state.activeId) ?? state.profiles[0]
  );
}

export async function loadProfiles(): Promise<Profiles> {
  const store = await load(FILE, { defaults: {} });
  const saved = await store.get<Profiles>(KEY);
  if (saved?.profiles?.length) {
    return {
      // Profiles saved before paper moved to the printer get the default.
      profiles: saved.profiles.map((p) => ({ ...p, paper: p.paper ?? "80" })),
      activeId: saved.profiles.some((p) => p.id === saved.activeId)
        ? saved.activeId
        : saved.profiles[0].id,
    };
  }

  // First run after the single-printer version: keep those settings as
  // the matching seed profile.
  const legacy = await store.get<Partial<Printer>>(LEGACY_KEY);
  if (legacy?.kind) {
    const target = legacy.kind === "impact" ? "sp743" : "tsp800ii";
    const profiles = SEED.map((p) =>
      p.id === target ? { ...p, ...legacy, id: p.id, name: p.name } : p,
    );
    const state = { profiles, activeId: target };
    await saveProfiles(state);
    await store.delete(LEGACY_KEY);
    await store.save();
    return state;
  }
  return DEFAULT_PROFILES;
}

export async function saveProfiles(state: Profiles): Promise<void> {
  const store = await load(FILE, { defaults: {} });
  await store.set(KEY, state);
  await store.save();
}
