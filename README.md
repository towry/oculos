# OculOS

> **"If it's on the screen, it's an API."**

OculOS exposes any desktop application's UI as a REST API and MCP server. Built with Rust, powered by native OS accessibility APIs.

No screenshots. No pixel coordinates. No browser extensions. Just JSON.

## Quick Start

```bash
cargo build --release
./target/release/oculos            # HTTP API + Dashboard → http://127.0.0.1:7878
./target/release/oculos --mcp      # MCP server over stdio
```

## How It Works

OculOS reads the OS accessibility tree (Windows UIA, macOS AX, Linux AT-SPI2) and assigns each UI element a temporary `oculos_id`. Use that ID to interact:

```bash
# List windows
curl http://localhost:7878/windows

# Get UI tree
curl http://localhost:7878/windows/1234/tree

# Click a button
curl -X POST http://localhost:7878/interact/{id}/click

# Type into a field
curl -X POST http://localhost:7878/interact/{id}/set-text -d '{"text":"hello"}'
```

## API

| Endpoint | Description |
|----------|-------------|
| `GET /windows` | List visible windows |
| `GET /windows/{pid}/tree` | Full UI element tree |
| `GET /windows/{pid}/find?q=&type=&interactive=true` | Search elements |
| `GET /hwnd/{hwnd}/tree` | Tree by window handle |
| `POST /windows/{pid}/focus` | Focus window |
| `POST /windows/{pid}/close` | Close window |
| `POST /interact/{id}/click` | Click element |
| `POST /interact/{id}/set-text` | Set text `{"text":"..."}` |
| `POST /interact/{id}/send-keys` | Send keys `{"keys":"{CTRL+A}hello{ENTER}"}` |
| `POST /interact/{id}/toggle` | Toggle checkbox |
| `POST /interact/{id}/expand` | Expand dropdown/tree |
| `POST /interact/{id}/collapse` | Collapse dropdown/tree |
| `POST /interact/{id}/select` | Select list item |
| `POST /interact/{id}/set-range` | Set slider `{"value":75}` |
| `POST /interact/{id}/scroll` | Scroll `{"direction":"down"}` |
| `POST /interact/{id}/highlight` | Highlight element on screen |
| `GET /ws` | WebSocket live events |
| `GET /health` | Server status, uptime, version |

## MCP Setup

Add to your MCP client config (Claude, Cursor, Windsurf, etc.):

```json
{
  "mcpServers": {
    "oculos": {
      "command": "/path/to/oculos",
      "args": ["--mcp"]
    }
  }
}
```

For HTTP-mode agents, paste [`AGENTS.md`](./AGENTS.md) into the system prompt.

## Dashboard

Built-in web dashboard at `http://127.0.0.1:7878` with:

- Window list with focus/close controls
- Interactive UI element tree with search
- Element inspector with all available actions
- Automation recorder — record interactions, export as Python/JS/curl
- JSON viewer with copy support
- WebSocket live event indicator
- Keyboard shortcuts: `R` refresh, `/` search, `E` expand, `C` collapse, `H` highlight

## Platform Support

| Platform | Backend | Status |
|----------|---------|--------|
| Windows | UI Automation via `windows-rs` | ✅ Full |
| Linux | AT-SPI2 via `atspi` + `zbus` | ✅ Working |
| macOS | Accessibility API | 🚧 Planned |

## CLI

```
oculos [OPTIONS]

  -b, --bind <ADDR>       Bind address [default: 127.0.0.1:7878]
      --static-dir <DIR>  Dashboard files [default: static]
      --log <LEVEL>       Log level [default: info]
      --mcp               Run as MCP server over stdio
```

## Contributing

PRs welcome. Main areas:

- **macOS backend** — `AXUIElement` implementation
- **Client SDKs** — Python, TypeScript
- **Tests** — cross-app integration tests

## License

MIT
