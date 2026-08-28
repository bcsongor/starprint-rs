import { load } from "@tauri-apps/plugin-store";
import type { Printer } from "./api";

const FILE = "settings.json";
const KEY = "printer";

export const DEFAULT_PRINTER: Printer = {
  kind: "thermal",
  host: "192.168.1.60",
  port: 9100,
  density: 3,
  speed: "slow",
  cut: true,
};

export async function loadPrinter(): Promise<Printer> {
  const store = await load(FILE, { defaults: {} });
  const saved = await store.get<Partial<Printer>>(KEY);
  const { kind, host, port, density, speed, cut } = {
    ...DEFAULT_PRINTER,
    ...saved,
  };
  return { kind, host, port, density, speed, cut };
}

export async function savePrinter(printer: Printer): Promise<void> {
  const store = await load(FILE, { defaults: {} });
  await store.set(KEY, printer);
  await store.save();
}
