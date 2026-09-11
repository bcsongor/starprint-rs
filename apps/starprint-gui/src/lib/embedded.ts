import { invoke } from "@tauri-apps/api/core";
import type { Server } from "./api";

/** Mirrors `Address` in src-tauri/src/api.rs. */
export interface Address {
  ip: string;
  /** The adapter, such as `Wi-Fi` or `en0`. */
  name: string;
}

/** Loopback first, then each adapter's IPv4 address. */
export function listAddresses() {
  return invoke<Address[]>("list_addresses");
}

/** Where the server listens. */
export interface Listen {
  ip: string;
  port: number;
}

export const LOOPBACK = "127.0.0.1";

/** Loopback on the command line's default port. */
export const DEFAULT_LISTEN: Listen = { ip: LOOPBACK, port: 9110 };

/**
 * Starts the server inside the app at `listen`, replacing one already
 * running, and returns where it is and the token it wants.
 */
export function startServer(listen: Listen) {
  return invoke<Server>("start_server", { ...listen });
}
