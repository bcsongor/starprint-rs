import { load } from "@tauri-apps/plugin-store";
import type { Printer } from "./api";

const FILE = "settings.json";
const KEY = "profiles";
const LINEAR_KEY = "linear";
/** Whether the HTTP API runs inside the app. */
const API_KEY = "api";
/** From before profiles existed; migrated on first load. */
const LEGACY_KEY = "printer";

export interface Profile extends Printer {
  id: string;
  name: string;
}

export interface Profiles {
  profiles: Profile[];
  activeId: string;
}

/** A Linear account; absent until a key is pasted in. */
export interface Linear {
  /** A personal API key, kept in the settings file as it is. */
  apiKey: string;
  /** Whose key it is while connected. Null once disconnected; the key
   * stays so reconnecting is a click. */
  user: string | null;
  /** Whether newly assigned issues print as they arrive. */
  autoPrint: boolean;
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
  { ...THERMAL, id: "tsp700ii", name: "TSP700II" },
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

export function toPrinter(profile: Profile): Printer {
  const { id: _id, name: _name, ...printer } = profile;
  return printer;
}

export function activeProfile(state: Profiles): Profile {
  return (
    state.profiles.find((p) => p.id === state.activeId) ?? state.profiles[0]
  );
}

async function read<T>(key: string): Promise<T | undefined> {
  const store = await load(FILE, { defaults: {} });
  return store.get<T>(key);
}

async function write(key: string, value: unknown): Promise<void> {
  const store = await load(FILE, { defaults: {} });
  await store.set(key, value);
  await store.save();
}

export async function loadProfiles(): Promise<Profiles> {
  const store = await load(FILE, { defaults: {} });
  const saved = await store.get<Profiles>(KEY);
  if (saved?.profiles?.length) {
    return {
      // Profiles saved before paper existed.
      profiles: saved.profiles.map((p) => ({ ...p, paper: p.paper ?? "80" })),
      activeId: saved.profiles.some((p) => p.id === saved.activeId)
        ? saved.activeId
        : saved.profiles[0].id,
    };
  }

  // First run after the single-printer version.
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

export function saveProfiles(state: Profiles): Promise<void> {
  return write(KEY, state);
}

export async function loadLinear(): Promise<Linear | null> {
  return (await read<Linear>(LINEAR_KEY)) ?? null;
}

export function saveLinear(linear: Linear): Promise<void> {
  return write(LINEAR_KEY, linear);
}

export async function loadApiEnabled(): Promise<boolean> {
  return (await read<boolean>(API_KEY)) ?? false;
}

export function saveApiEnabled(enabled: boolean): Promise<void> {
  return write(API_KEY, enabled);
}
