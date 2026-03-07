"""Demonstrate the wait/poll endpoint — wait for an element to appear."""

import sys, os, requests
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sdk", "python"))

# Wait endpoint is not yet in the SDK, so we use requests directly.
BASE = "http://127.0.0.1:7878"

# Pick first window
r = requests.get(f"{BASE}/windows")
windows = r.json()["data"]
if not windows:
    print("No windows found.")
    sys.exit(1)

target = windows[0]
pid = target["pid"]
print(f"Target: {target['title']} (PID {pid})")

# Wait for a Button to appear (should be instant for most apps)
print("Waiting for a Button element (timeout: 3s)...")

r = requests.get(f"{BASE}/windows/{pid}/wait", params={
    "type": "Button",
    "interactive": "true",
    "timeout": "3000",
})

if r.status_code == 200:
    data = r.json()["data"]
    print(f"Found {len(data)} buttons!")
    for btn in data[:5]:
        print(f"  - {btn['label']}")
elif r.status_code == 408:
    print("Timeout — no matching element found.")
else:
    print(f"Error: {r.status_code} — {r.text}")
