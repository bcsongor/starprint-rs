/**
 * What went wrong, for a toast or the preview. An `Error` gives its
 * message, which `String` would put "Error: " before; a Tauri command
 * rejects with the text itself.
 */
export function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
