import type { Server } from "@/lib/api";

/**
 * A client's whole config with this server in it, under the
 * `mcpServers` key most clients use, so it pastes into an empty file
 * as it is.
 */
function mcpConfig(starprint: object): string {
  return JSON.stringify({ mcpServers: { starprint } }, null, 2);
}

/** For a client that speaks HTTP itself, with the token to send. */
export function httpConfig({ url, token }: Server): string {
  return mcpConfig({
    type: "http",
    url: `${url}/mcp`,
    headers: { Authorization: `Bearer ${token}` },
  });
}

/**
 * For Claude Desktop, whose config only runs local commands, so
 * `mcp-remote` bridges it to `/mcp`. Desktop on Windows splits an
 * argument at a space, so the header has none and `mcp-remote` fills
 * the token in from the environment. `--allow-http` lets it reach an
 * address that is not loopback.
 */
export function desktopConfig({ url, token }: Server): string {
  return mcpConfig({
    command: "npx",
    args: [
      "-y",
      "mcp-remote",
      `${url}/mcp`,
      "--allow-http",
      "--transport",
      "http-only",
      "--header",
      "Authorization:${AUTH_HEADER}",
    ],
    env: { AUTH_HEADER: `Bearer ${token}` },
  });
}
