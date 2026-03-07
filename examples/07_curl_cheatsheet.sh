#!/bin/bash
# OculOS curl cheatsheet — every endpoint in one file.
# Make sure OculOS is running: ./target/release/oculos

BASE="http://127.0.0.1:7878"

# ── Health ────────────────────────────────────────────────────────────────────
echo "=== Health ==="
curl -s "$BASE/health" | python3 -m json.tool

# ── List windows ──────────────────────────────────────────────────────────────
echo -e "\n=== Windows ==="
curl -s "$BASE/windows" | python3 -m json.tool

# ── Get UI tree (replace PID) ────────────────────────────────────────────────
PID=12345
echo -e "\n=== UI Tree (PID=$PID) ==="
curl -s "$BASE/windows/$PID/tree" | python3 -m json.tool | head -50

# ── Find elements ────────────────────────────────────────────────────────────
echo -e "\n=== Find Buttons ==="
curl -s "$BASE/windows/$PID/find?type=Button&interactive=true" | python3 -m json.tool

# ── Wait for element ─────────────────────────────────────────────────────────
echo -e "\n=== Wait for Submit button (5s timeout) ==="
curl -s "$BASE/windows/$PID/wait?q=Submit&type=Button&timeout=5000" | python3 -m json.tool

# ── Screenshot ───────────────────────────────────────────────────────────────
echo -e "\n=== Screenshot ==="
curl -s -o screenshot.png "$BASE/windows/$PID/screenshot"
echo "Saved screenshot.png ($(wc -c < screenshot.png) bytes)"

# ── Click ─────────────────────────────────────────────────────────────────────
ID="element-oculos-id-here"
echo -e "\n=== Click ==="
curl -s -X POST "$BASE/interact/$ID/click" | python3 -m json.tool

# ── Set text ──────────────────────────────────────────────────────────────────
echo -e "\n=== Set Text ==="
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"text":"Hello from OculOS"}' \
  "$BASE/interact/$ID/set-text" | python3 -m json.tool

# ── Send keys ─────────────────────────────────────────────────────────────────
echo -e "\n=== Send Keys ==="
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"keys":"{CTRL+A}new text{ENTER}"}' \
  "$BASE/interact/$ID/send-keys" | python3 -m json.tool

# ── Toggle checkbox ───────────────────────────────────────────────────────────
echo -e "\n=== Toggle ==="
curl -s -X POST "$BASE/interact/$ID/toggle" | python3 -m json.tool

# ── Expand / Collapse ────────────────────────────────────────────────────────
echo -e "\n=== Expand ==="
curl -s -X POST "$BASE/interact/$ID/expand" | python3 -m json.tool

# ── Select ────────────────────────────────────────────────────────────────────
echo -e "\n=== Select ==="
curl -s -X POST "$BASE/interact/$ID/select" | python3 -m json.tool

# ── Set range (slider) ───────────────────────────────────────────────────────
echo -e "\n=== Set Range ==="
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"value":75}' \
  "$BASE/interact/$ID/set-range" | python3 -m json.tool

# ── Scroll ────────────────────────────────────────────────────────────────────
echo -e "\n=== Scroll Down ==="
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"direction":"down"}' \
  "$BASE/interact/$ID/scroll" | python3 -m json.tool

# ── Highlight ─────────────────────────────────────────────────────────────────
echo -e "\n=== Highlight ==="
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"duration_ms":2000}' \
  "$BASE/interact/$ID/highlight" | python3 -m json.tool

# ── Batch ─────────────────────────────────────────────────────────────────────
echo -e "\n=== Batch ==="
curl -s -X POST -H "Content-Type: application/json" \
  -d '{"actions":[{"element_id":"id1","action":"click"},{"element_id":"id2","action":"focus"}]}' \
  "$BASE/interact/batch" | python3 -m json.tool

# ── Focus window ──────────────────────────────────────────────────────────────
echo -e "\n=== Focus Window ==="
curl -s -X POST "$BASE/windows/$PID/focus" | python3 -m json.tool

# ── Close window ──────────────────────────────────────────────────────────────
echo -e "\n=== Close Window ==="
curl -s -X POST "$BASE/windows/$PID/close" | python3 -m json.tool
