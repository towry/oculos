"""List all open windows and their details."""

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sdk", "python"))

from oculos import OculOS

client = OculOS()

windows = client.list_windows()
print(f"Found {len(windows)} windows:\n")

for w in windows:
    print(f"  PID: {w['pid']:>6}  HWND: {w['hwnd']:>10}  {w['exe_name']:<25} {w['title']}")
