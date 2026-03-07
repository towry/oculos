"""Find a button in Calculator and click it."""

import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "sdk", "python"))

from oculos import OculOS

client = OculOS()

# Find Calculator window
windows = client.list_windows()
calc = next((w for w in windows if "calc" in w["exe_name"].lower()), None)

if not calc:
    print("Calculator not found. Open Calculator first.")
    sys.exit(1)

pid = calc["pid"]
print(f"Found Calculator — PID {pid}")

# Find the "5" button
buttons = client.find_elements(pid, query="5", element_type="Button")
if not buttons:
    print("Button '5' not found.")
    sys.exit(1)

btn = buttons[0]
print(f"Found button: '{btn['label']}' (id: {btn['oculos_id'][:8]}...)")

# Click it
client.click(btn["oculos_id"])
print("Clicked!")
