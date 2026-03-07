# oculos-sdk

Python SDK for [OculOS](https://github.com/huseyinstif/oculos) — control any desktop app through JSON.

## Install

```bash
# From the repo root:
cd sdk/python
pip install .
```

> PyPI package (`pip install oculos-sdk`) coming soon.

## Quick Start

```python
from oculos import OculOS

client = OculOS()  # default: http://127.0.0.1:7878

# List windows
windows = client.list_windows()
for w in windows:
    print(f"{w['pid']}  {w['title']}")

# Find a button
buttons = client.find_elements(pid, query="Submit", element_type="Button")

# Click it
client.click(buttons[0]["oculos_id"])

# Type into a text field
client.set_text(element_id, "hello world")

# Send keyboard shortcuts
client.send_keys(element_id, "{CTRL+A}new text{ENTER}")

# Highlight an element on screen
client.highlight(element_id)
```

## All Methods

| Method | Description |
|--------|-------------|
| `list_windows()` | List all visible windows |
| `get_tree(pid)` | Full UI element tree |
| `find_elements(pid, query=, element_type=, interactive=)` | Search elements |
| `focus_window(pid)` | Bring window to foreground |
| `close_window(pid)` | Close window |
| `click(id)` | Click element |
| `set_text(id, text)` | Set text content |
| `send_keys(id, keys)` | Keyboard input |
| `focus(id)` | Move focus |
| `toggle(id)` | Toggle checkbox |
| `expand(id)` | Expand dropdown/tree |
| `collapse(id)` | Collapse |
| `select(id)` | Select list item |
| `set_range(id, value)` | Set slider value |
| `scroll(id, direction)` | Scroll container |
| `scroll_into_view(id)` | Scroll into viewport |
| `highlight(id, duration_ms=)` | Highlight on screen |
| `health()` | Server status |

## Requirements

- Python 3.9+
- OculOS server running (`oculos` binary)

## License

MIT
