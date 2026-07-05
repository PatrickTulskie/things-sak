# macOS launchd examples for the streamable HTTP MCP server

These examples show how to keep `things-sak serve` running on a Mac so remote
MCP clients can connect to Things over streamable HTTP.

Use the LaunchAgent for normal desktop use. Things is a GUI app, and the
AppleScript/URL-scheme integration is most reliable when `things-sak` runs in
the logged-in user's session.

## 1. Install the binary

On the Mac that has Things installed:

```sh
cargo install things-sak
```

The LaunchAgent below expects the binary at
`$HOME/.cargo/bin/things-sak`, which is Cargo's default install path.

## 2. Create an environment file

Copy the sample environment file and edit the values:

```sh
mkdir -p "$HOME/Library/Application Support/things-sak"
cp examples/macos/things-sak.env.example \
  "$HOME/Library/Application Support/things-sak/things-sak.env"
chmod 600 "$HOME/Library/Application Support/things-sak/things-sak.env"
$EDITOR "$HOME/Library/Application Support/things-sak/things-sak.env"
```

Required values:

- `THINGS_SAK_TOKEN`: bearer token MCP clients must send.
- `THINGS_SAK_BIND`: the Mac's LAN address, or `127.0.0.1` for local-only use.
- `THINGS_SAK_AUTH_TOKEN`: optional, but required for update/complete/move tools.

Get `THINGS_SAK_AUTH_TOKEN` from Things:

1. Things → **Settings → General → Enable Things URLs → Manage**
2. Copy the token.

## 3. Install the LaunchAgent

```sh
mkdir -p "$HOME/Library/LaunchAgents" "$HOME/Library/Logs/things-sak"
cp examples/macos/com.example.things-sak.plist \
  "$HOME/Library/LaunchAgents/com.example.things-sak.plist"

launchctl bootstrap "gui/$(id -u)" \
  "$HOME/Library/LaunchAgents/com.example.things-sak.plist"
launchctl enable "gui/$(id -u)/com.example.things-sak"
launchctl kickstart -k "gui/$(id -u)/com.example.things-sak"
```

Check status and logs:

```sh
launchctl print "gui/$(id -u)/com.example.things-sak"
tail -f "$HOME/Library/Logs/things-sak/things-sak.err.log"
curl -fsS "http://127.0.0.1:32123/health"
```

Unload it:

```sh
launchctl bootout "gui/$(id -u)" \
  "$HOME/Library/LaunchAgents/com.example.things-sak.plist"
```

## MCP client configuration

Use this from another MCP client after replacing the host and token:

```json
{
  "mcpServers": {
    "things": {
      "type": "http",
      "url": "http://192.168.1.20:32123/mcp",
      "headers": { "Authorization": "Bearer change-me" }
    }
  }
}
```

## Optional LaunchDaemon

`com.example.things-sak.daemon.plist` is included as a reference for admins who
need a system-level daemon. It expects a system install at `/usr/local/bin`, for
example:

```sh
sudo cargo install --root /usr/local things-sak
```

Prefer the LaunchAgent unless you know your Things AppleScript permissions and
GUI-session access are configured correctly. Running as root is not a personality
trait.
