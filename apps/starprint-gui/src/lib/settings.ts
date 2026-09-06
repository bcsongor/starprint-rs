import { load } from "@tauri-apps/plugin-store";
import type { Listen, Printer } from "./api";

const FILE = "settings.json";
const KEY = "profiles";
const LINEAR_KEY = "linear";
/** Whether the HTTP API runs inside the app. */
const API_KEY = "api";
/** The bearer token the API requires; made once, kept so clients stay set up. */
const API_TOKEN_KEY = "apiToken";
/** Where the API listens. */
const API_LISTEN_KEY = "apiListen";

/** Loopback on the standalone server's port. */
export const DEFAULT_LISTEN: Listen = { ip: "127.0.0.1", port: 9110 };

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
  { ...THERMAL, id: "tsp800ii", name: "TSP800II" },
  { ...THERMAL, id: "sp743", name: "SP743", kind: "impact" },
];

export const DEFAULT_PROFILES: Profiles = {
  profiles: SEED,
  activeId: "tsp800ii",
};

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
  const saved = await read<Profiles>(KEY);
  if (!saved?.profiles?.length) return DEFAULT_PROFILES;
  return {
    profiles: saved.profiles,
    activeId: saved.profiles.some((p) => p.id === saved.activeId)
      ? saved.activeId
      : saved.profiles[0].id,
  };
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

export async function loadApiListen(): Promise<Listen> {
  return (await read<Listen>(API_LISTEN_KEY)) ?? DEFAULT_LISTEN;
}

export function saveApiListen(listen: Listen): Promise<void> {
  return write(API_LISTEN_KEY, listen);
}

/** The API token, generated on first use and kept from then on. */
export async function loadApiToken(): Promise<string> {
  const saved = await read<string>(API_TOKEN_KEY);
  if (saved) return saved;
  const token = crypto.randomUUID();
  await write(API_TOKEN_KEY, token);
  return token;
}
