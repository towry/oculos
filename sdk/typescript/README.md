# oculos-sdk

TypeScript SDK for [OculOS](https://github.com/huseyinstif/oculos) — control any desktop app through JSON.

## Install

```bash
npm install oculos-sdk
```

## Quick Start

```typescript
import { OculOS } from "oculos-sdk";

const client = new OculOS(); // default: http://127.0.0.1:7878

// List windows
const windows = await client.listWindows();
windows.forEach((w) => console.log(`${w.pid}  ${w.title}`));

// Find a button
const buttons = await client.findElements(pid, { query: "Submit", type: "Button" });

// Click it
await client.click(buttons[0].oculos_id);

// Type into a text field
await client.setText(elementId, "hello world");

// Send keyboard shortcuts
await client.sendKeys(elementId, "{CTRL+A}new text{ENTER}");

// Highlight an element on screen
await client.highlight(elementId);
```

## All Methods

| Method | Description |
|--------|-------------|
| `listWindows()` | List all visible windows |
| `getTree(pid)` | Full UI element tree |
| `findElements(pid, opts?)` | Search elements |
| `focusWindow(pid)` | Bring window to foreground |
| `closeWindow(pid)` | Close window |
| `click(id)` | Click element |
| `setText(id, text)` | Set text content |
| `sendKeys(id, keys)` | Keyboard input |
| `focus(id)` | Move focus |
| `toggle(id)` | Toggle checkbox |
| `expand(id)` | Expand dropdown/tree |
| `collapse(id)` | Collapse |
| `select(id)` | Select list item |
| `setRange(id, value)` | Set slider value |
| `scroll(id, direction)` | Scroll container |
| `scrollIntoView(id)` | Scroll into viewport |
| `highlight(id, durationMs?)` | Highlight on screen |
| `health()` | Server status |

## Requirements

- Node.js 18+
- OculOS server running (`oculos` binary)

## License

MIT
