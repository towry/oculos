"""Demonstrate batch operations — multiple actions in one request."""

import sys, os, json
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sdk", "python"))

from oculos import OculOS

client = OculOS()

# Find a window with interactive elements
windows = client.list_windows()
if not windows:
    print("No windows found.")
    sys.exit(1)

target = windows[0]
pid = target["pid"]
print(f"Target: {target['title']} (PID {pid})")

# Find first 3 buttons
buttons = client.find_elements(pid, element_type="Button", interactive=True)
if len(buttons) < 2:
    print("Not enough buttons found for batch demo.")
    sys.exit(1)

# Build batch payload — highlight first 3 buttons in sequence
import requests

actions = []
for btn in buttons[:3]:
    actions.append({
        "element_id": btn["oculos_id"],
        "action": "focus",
    })

print(f"Sending batch with {len(actions)} actions...")

r = requests.post("http://127.0.0.1:7878/interact/batch", json={"actions": actions})
data = r.json()

for result in data["data"]:
    status = "✅" if result["success"] else f"❌ {result['error']}"
    print(f"  [{result['index']}] {result['action']} → {status}")

print("\nDone!")
