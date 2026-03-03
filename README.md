# OculOS

> **"If it's on the screen, it's an API."**

OculOS is a lightweight, open-source daemon that exposes any desktop application's UI as a JSON REST API — and as an **MCP server** for AI agents.

Control Notepad, Chrome, Teams, VS Code, or any other application programmatically. No browser extensions. No code injection. No screen coordinates. Just JSON.

---

## How it works

OculOS uses the operating system's native accessibility layer:

- **Windows** — UI Automation (UIA) via `windows-rs`
- **macOS** — Accessibility API (`AXUIElement`) via `accessibility` + `core-graphics`
- **Linux** — AT-SPI2 via D-Bus (`atspi` + `zbus`), keyboard via `xdotool`

Every button, text field, checkbox, and menu item gets a temporary UUID (`oculos_id`). You use that ID to click, type, toggle, scroll — whatever the element supports.

---

## Quick Start

### Build

```bash
git clone https://github.com/oculos-project/oculos
cd oculos
cargo build --release
```

### Run as HTTP API server

```bash
./target/release/oculos
# Dashboard → http://127.0.0.1:7878
# API       → http://127.0.0.1:7878/windows
```

### Run as MCP server (for AI agents)

```bash
./target/release/oculos --mcp
```

---

## Connecting to AI Agents

### Claude Code / Claude Desktop

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "oculos": {
      "command": "C:/path/to/oculos.exe",
      "args": ["--mcp"]
    }
  }
}
```

Done. Claude now has tools: `list_windows`, `get_ui_tree`, `find_elements`, `click_element`, `set_text`, `send_keys`, and 12 more — all with full descriptions and schemas.

### Cursor / Windsurf

Same config — add to your IDE's MCP servers list:

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

### OpenAI / Gemini / Custom agents (HTTP mode)

1. Start OculOS in HTTP mode: `oculos`
2. Paste [`AGENTS.md`](./AGENTS.md) into your agent's system prompt
3. Give the agent a tool to make HTTP requests (curl, fetch, requests…)
4. The agent can now discover and control any running application

---

## API Reference

### Discovery

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/windows` | List all visible windows |
| GET | `/windows/{pid}/tree` | Full UI tree for a process |
| GET | `/windows/{pid}/find?q=&type=&interactive=true` | Search elements |
| GET | `/hwnd/{hwnd}/tree` | UI tree for a specific window handle |
| GET | `/hwnd/{hwnd}/find` | Search in a specific window |

Use the `/hwnd/` variants when a process has multiple windows with the same PID (e.g. Teams, multi-window editors).

### Window Operations

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/windows/{pid}/focus` | Bring window to foreground |
| POST | `/windows/{pid}/close` | Close window gracefully |

### Element Interactions

| Method | Endpoint | Body | Description |
|--------|----------|------|-------------|
| POST | `/interact/{id}/click` | — | Click (button, link, menu item) |
| POST | `/interact/{id}/set-text` | `{"text":"…"}` | Replace field content |
| POST | `/interact/{id}/send-keys` | `{"keys":"…"}` | Keyboard simulation |
| POST | `/interact/{id}/focus` | — | Move keyboard focus |
| POST | `/interact/{id}/toggle` | — | CheckBox / ToggleButton |
| POST | `/interact/{id}/expand` | — | ComboBox / TreeItem / MenuItem |
| POST | `/interact/{id}/collapse` | — | ComboBox / TreeItem / MenuItem |
| POST | `/interact/{id}/select` | — | ListItem / RadioButton / TabItem |
| POST | `/interact/{id}/set-range` | `{"value":75}` | Slider / Spinner |
| POST | `/interact/{id}/scroll` | `{"direction":"down"}` | Scroll container |
| POST | `/interact/{id}/scroll-into-view` | — | Scroll element into viewport |

#### Special keys for send-keys

```
{ENTER}  {TAB}  {ESC}  {DELETE}  {BACKSPACE}
{UP}  {DOWN}  {LEFT}  {RIGHT}  {HOME}  {END}  {PGUP}  {PGDN}
{F1}–{F12}
{CTRL+A}  {CTRL+C}  {CTRL+V}  {CTRL+X}  {CTRL+Z}  {CTRL+Y}
{ALT+F4}  {WIN+D}
```

---

## The `actions` field

Every element has an `actions` list that tells you exactly what you can do with it:

```json
{
  "oculos_id": "a3f8c2d1-...",
  "type": "Button",
  "label": "Submit",
  "enabled": true,
  "actions": ["click", "focus"],
  "rect": { "x": 120, "y": 340, "width": 80, "height": 32 }
}
```

If `actions` is empty, the element is display-only. AI agents use this field to decide what to do without any guessing.

---

## App Compatibility

### Windows
| App type | Support | Notes |
|----------|---------|-------|
| Win32 (Notepad, Explorer, Office) | Full | |
| WPF / .NET | Full | |
| Electron (Chrome, Teams, VS Code, Slack) | Good | Shallow but functional |
| Qt | Good | |
| Terminal apps | Partial | UI chrome yes; buffer limited |
| UWP / WinUI 3 (Settings, Store) | Limited | OS sandbox restricts UIA |

### macOS
| App type | Support | Notes |
|----------|---------|-------|
| Native Cocoa (Finder, Safari, TextEdit) | Good | Via AXUIElement Accessibility API |
| Electron (Chrome, VS Code, Slack) | Good | Accessibility tree exposed |
| SwiftUI | Good | Most elements accessible |
| Qt | Partial | Depends on Qt accessibility bridge |
| Sandboxed apps (App Store) | Limited | May require extra entitlements |

> **Note:** macOS requires Accessibility permission. Go to **System Settings → Privacy & Security → Accessibility** and enable OculOS.

### Linux
| App type | Support | Notes |
|----------|---------|-------|
| GTK3 / GTK4 (GNOME apps) | Good | AT-SPI2 well supported |
| Qt5 / Qt6 (KDE apps) | Good | Via Qt accessibility bridge |
| Electron (Chrome, VS Code, Slack) | Good | AT-SPI2 exposed |
| Terminal emulators | Partial | UI chrome accessible, buffer limited |
| X11 apps | Good | xdotool for keyboard/window ops |
| Wayland-only apps | Limited | xdotool fallback may not work |

> **Note:** Linux requires `at-spi2-core` running and optionally `xdotool` for keyboard simulation and window management.

---

## CLI Options

```
oculos [OPTIONS]

Options:
  -b, --bind <ADDR>         Bind address [default: 127.0.0.1:7878]
      --static-dir <DIR>    Dashboard static files [default: static]
      --log <LEVEL>         Log level: trace/debug/info/warn/error [default: info]
      --mcp                 Run as MCP server over stdin/stdout
  -h, --help                Print help
```

---

## Architecture

```
oculos/
├── src/
│   ├── main.rs          # Entry point, CLI args, server startup
│   ├── mcp.rs           # MCP server — JSON-RPC 2.0 over stdio
│   ├── types.rs         # UiElement, WindowInfo, ElementType, etc.
│   ├── platform/
│   │   ├── mod.rs       # UiBackend trait
│   │   ├── windows.rs   # Windows UIA implementation (complete)
│   │   ├── macos.rs     # macOS stub (TODO)
│   │   └── linux.rs     # Linux AT-SPI stub (TODO)
│   └── api/
│       ├── mod.rs       # Axum router
│       ├── windows.rs   # Discovery & window operation handlers
│       └── interact.rs  # Element interaction handlers
├── static/
│   └── index.html       # Web dashboard (dark theme)
├── AGENTS.md            # System prompt for non-MCP AI agents
└── Cargo.toml
```

---

## How OculOS Compares

There are many desktop automation and computer-use tools out there. Here's how OculOS differs:

| Tool | Approach | Scope | OculOS Difference |
|------|----------|-------|-------------------|
| **[Microsoft UFO³](https://github.com/microsoft/UFO)** | Windows UIA + Vision LLM hybrid | Full AI agent with its own LLM | OculOS is **infrastructure, not an agent** — it exposes the UI as an API and lets you bring your own AI model. UFO bundles its own reasoning; OculOS is the layer underneath. |
| **[Playwright MCP](https://github.com/microsoft/playwright-mcp)** | Accessibility tree + MCP | Browser only | Same philosophy (a11y tree → structured API), but Playwright only works inside browsers. OculOS targets **any desktop application**. |
| **[AskUI Vision Agent](https://github.com/askui/vision-agent)** | Vision-based + accessibility | Cross-platform, commercial focus | Heavier framework with commercial licensing. OculOS is MIT-licensed, minimal, and Rust-native. |
| **[Anthropic Computer Use](https://www.anthropic.com/news/3-5-models-and-computer-use)** | Screenshot + pixel coordinates | Cloud-dependent AI | Requires sending screenshots to Anthropic's API. OculOS works **100 % locally**, no screenshots, no cloud, no latency. |
| **[Cua](https://github.com/trycua/cua)** | Vision-based CUA framework | macOS/Linux sandboxes | Designed for sandboxed VMs. OculOS runs on the **host OS** directly — no VM required. |
| **[Agent TARS](https://github.com/bytedance/UI-TARS-desktop)** | Vision model (UI-TARS) | Desktop + browser | Relies on vision models to understand the screen. OculOS uses the **OS accessibility tree** — deterministic, zero inference cost, instant. |
| **[pywinauto](https://github.com/pywinauto/pywinauto)** | Windows UIA / Win32 bindings | Python library | Same underlying API, but pywinauto is a Python scripting library. OculOS wraps it into a **REST + MCP server** that any language or AI agent can call. |
| **[FlaUI](https://github.com/FlaUI/FlaUI)** | Windows UIA wrapper | C# / .NET library | .NET-only, no server mode. OculOS is a standalone binary with HTTP + MCP. |
| **[nut.js](https://github.com/nut-tree/nut.js)** | Native UI automation | JS/TS library | Coordinate/image-based. OculOS is **semantic** — it works with element types, labels, and actions, not pixel positions. |
| **[PyAutoGUI](https://github.com/asweigart/pyautogui)** | Pixel/coordinate automation | Python, cross-platform | Fragile screen-coordinate based clicks. OculOS uses the **accessibility tree** — no resolution dependency, no pixel hunting. |

### OculOS in a nutshell

- **Not an agent** — it's the **tool layer** that agents use. Bring Claude, GPT, Gemini, or your own model.
- **Not vision-based** — uses the OS accessibility tree. Deterministic, fast, zero GPU cost.
- **Not a library** — it's a **server**. REST API + MCP protocol. Any language, any framework.
- **Not cloud-dependent** — everything runs locally. Your data never leaves your machine.
- **Rust-native** — single binary, ~3 MB, no runtime dependencies.

---

## Roadmap

### Completed
- [x] Windows UIA backend (full)
- [x] REST API server with Axum
- [x] MCP server (JSON-RPC 2.0 over stdio)
- [x] Web dashboard
- [x] Element registry with session-scoped UUIDs
- [x] Full keyboard simulation engine

### In Progress
- [ ] **macOS backend** — Accessibility API via `AXUIElement`
- [ ] **Linux backend** — AT-SPI2 via D-Bus

### Planned
- [ ] **UWP/WinUI workarounds** — alternative access paths for sandboxed Windows apps
- [ ] **Screenshot fallback** — OCR-based element detection for apps without an accessibility tree
- [ ] **Element caching & diffing** — watch for UI changes and emit events (WebSocket/SSE)
- [ ] **Action recording & replay** — record a sequence of interactions and replay them
- [ ] **Multi-monitor support** — coordinate mapping across displays
- [ ] **Element highlighting** — overlay API to visually mark elements on screen
- [ ] **Session persistence** — save/restore element registry across restarts
- [ ] **Plugin system** — custom interaction patterns for specific apps (e.g. terminal buffer reading)
- [ ] **Docker image** — containerised OculOS for CI/CD pipelines
- [ ] **Python & TypeScript SDKs** — typed client libraries for the REST API
- [ ] **Batch operations** — execute multiple interactions in a single request
- [ ] **Conditional waits** — wait for an element to appear/disappear before acting
- [ ] **Performance profiling endpoint** — measure tree-build times per application

---

## Contributing

We welcome contributions! Top areas where help is needed:

1. **macOS backend** — implementing `AXUIElement` via the Accessibility API
2. **Linux backend** — implementing AT-SPI2 via D-Bus / `atspi` crate
3. **UWP/WinUI support** — investigating alternative access paths
4. **Client SDKs** — Python and TypeScript client libraries
5. **Tests** — integration tests across different application types

---

## License

MIT

---

*OculOS — Because every pixel deserves an endpoint.*
