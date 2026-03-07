"""Open Notepad's text area and type into it."""

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sdk", "python"))

from oculos import OculOS

client = OculOS()

# Find Notepad window
windows = client.list_windows()
notepad = next((w for w in windows if "notepad" in w["exe_name"].lower()), None)

if not notepad:
    print("Notepad not found. Open Notepad first.")
    sys.exit(1)

pid = notepad["pid"]
print(f"Found Notepad — PID {pid}")

# Focus the window
client.focus_window(pid)

# Find the text editor area
editors = client.find_elements(pid, element_type="Edit", interactive=True)
if not editors:
    editors = client.find_elements(pid, element_type="Document", interactive=True)
if not editors:
    print("No text area found.")
    sys.exit(1)

editor = editors[0]
print(f"Found editor: type={editor['type']} (id: {editor['oculos_id'][:8]}...)")

# Type some text
client.set_text(editor["oculos_id"], "Hello from OculOS! 🚀")
print("Text set!")
