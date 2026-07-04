# things-sak — Things Swiss Army Knife

CLI and MCP server for [Things 3](https://culturedcode.com/things/) on macOS, written in Rust.

## How it talks to Things

- **Reads** use read-only AppleScript queries (`osascript`).
- **Writes** use the official [Things URL scheme](https://culturedcode.com/things/support/articles/2803573/) (`things:///add`, `things:///update`, …).
- The Things database is **never** touched directly.
- The only AppleScript writes are the few operations the URL scheme cannot express (move to Inbox, detach from project/area) — still via the official scripting API.

## Setup

```sh
cargo build --release
# binary at target/release/things-sak
```

Updating existing items (`done`, `update`, `move`, `project move`) requires the Things URL scheme auth token:

1. Things → **Settings → General → Enable Things URLs → Manage**
2. Copy the token and export it:

```sh
export THINGS_SAK_AUTH_TOKEN=<token>
```

Creating todos/projects and all reads work without the token.

## CLI

```sh
things-sak list                        # today (default)
things-sak list inbox --limit 20
things-sak add "Buy milk" -w today -d 2026-07-10 -t errand -l Groceries
things-sak done "Buy milk"
things-sak update "Buy milk" --append-notes "2%" --when tomorrow
things-sak move today "Buy milk"       # or a project/area title as destination
things-sak search milk
things-sak project list
things-sak project todos "Groceries"
things-sak tags
things-sak areas
things-sak --json list today           # JSON output for scripting
```

## MCP server

### stdio (same machine)

```json
{
  "mcpServers": {
    "things": { "command": "/path/to/things-sak", "args": ["mcp"] }
  }
}
```

### Streamable HTTP (over the network)

Run on the Mac that has Things:

```sh
things-sak serve --bind 192.168.1.20          # your workstation's LAN address
```

Binding beyond loopback **requires Bearer token auth** — a token is generated
and printed at startup if you don't pass one. Pin a stable token with
`--token` or `THINGS_SAK_TOKEN`. On the client (e.g. a Raspberry Pi):

```json
{
  "mcpServers": {
    "things": {
      "type": "http",
      "url": "http://192.168.1.20:32123/mcp",
      "headers": { "Authorization": "Bearer <token>" }
    }
  }
}
```

Notes:

- Default bind is `127.0.0.1` (loopback), default port `32123`.
- `--no-token` is refused on non-loopback binds.
- Tokens are only accepted in the `Authorization` header, never in URLs.
- The `Host` header is validated (DNS-rebinding protection). When binding a
  specific LAN address it is allowed automatically; add extra hostnames with
  `--allowed-host my-mac.local:32123`.
- For update tools to work, `THINGS_SAK_AUTH_TOKEN` must be set in the
  server's environment.
- No tunneling to the public internet is built in, on purpose. If you ever
  need remote access, put it behind a VPN (Tailscale/WireGuard).

## MCP tools

| Tool | Kind |
|------|------|
| `list_todos`, `search_todos`, `get_project_todos`, `list_projects`, `list_areas`, `list_tags` | read-only |
| `create_todo`, `create_project` | write (no auth token needed) |
| `update_todo`, `complete_todo`, `move_todo`, `move_project_to_area` | write (needs auth token) |
| `remove_todo_from_project`, `remove_project_from_area` | write (AppleScript) |

Reads return JSON including item `id`s; updates prefer ids but accept names.
Todos have a schedule (`when`) and a separate optional `deadline`.
