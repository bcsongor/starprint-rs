import { load } from "@tauri-apps/plugin-store";
import { DEFAULT_LISTEN, type Listen } from "./embedded";

const FILE = "settings.json";

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

/** What the app keeps for itself; profiles and schedules are the server's. */
export interface Settings {
  /** Whether the server also listens on the LAN, at `listen`. */
  lan: boolean;
  listen: Listen;
  /** The profile the picker shows. */
  printer: string | null;
  linear: Linear | null;
}

const DEFAULTS: Settings = {
  lan: false,
  listen: DEFAULT_LISTEN,
  printer: null,
  linear: null,
};

/** Keys from earlier versions ride along unread. */
export async function loadSettings(): Promise<Settings> {
  const store = await load(FILE, { defaults: {} });
  const saved = Object.fromEntries(await store.entries()) as Partial<Settings>;
  return { ...DEFAULTS, ...saved };
}

export async function saveSettings(changes: Partial<Settings>): Promise<void> {
  const store = await load(FILE, { defaults: {} });
  for (const [key, value] of Object.entries(changes)) {
    await store.set(key, value);
  }
  await store.save();
}
