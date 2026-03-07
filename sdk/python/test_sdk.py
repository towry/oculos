"""OculOS Python SDK — integration test."""

import sys
import os

# Add SDK to path
sys.path.insert(0, os.path.dirname(__file__))

from oculos import OculOS
from oculos.client import OculOSError

def test():
    client = OculOS()
    passed = 0
    failed = 0

    # ── 1. health() ──
    try:
        h = client.health()
        assert h["status"] == "running", f"Expected 'running', got {h['status']}"
        assert "version" in h
        assert "uptime_secs" in h
        print(f"  ✓ health() — status={h['status']}, version={h['version']}")
        passed += 1
    except Exception as e:
        print(f"  ✗ health() — {e}")
        failed += 1

    # ── 2. list_windows() ──
    try:
        windows = client.list_windows()
        assert isinstance(windows, list), "Expected list"
        assert len(windows) > 0, "No windows found"
        w = windows[0]
        assert "pid" in w
        assert "hwnd" in w
        assert "title" in w
        assert "exe_name" in w
        print(f"  ✓ list_windows() — {len(windows)} windows found")
        passed += 1
    except Exception as e:
        print(f"  ✗ list_windows() — {e}")
        failed += 1

    # Pick a window for further tests
    pid = windows[0]["pid"] if windows else None
    hwnd = windows[0]["hwnd"] if windows else None

    # ── 3. get_tree(pid) ──
    try:
        tree = client.get_tree(pid)
        assert tree is not None
        assert "oculos_id" in tree
        assert "type" in tree
        assert "children" in tree
        print(f"  ✓ get_tree({pid}) — root type={tree['type']}, children={len(tree['children'])}")
        passed += 1
    except Exception as e:
        print(f"  ✗ get_tree({pid}) — {e}")
        failed += 1

    # ── 4. get_tree_hwnd(hwnd) ──
    try:
        tree2 = client.get_tree_hwnd(hwnd)
        assert tree2 is not None
        assert "oculos_id" in tree2
        print(f"  ✓ get_tree_hwnd({hwnd}) — root type={tree2['type']}")
        passed += 1
    except Exception as e:
        print(f"  ✗ get_tree_hwnd({hwnd}) — {e}")
        failed += 1

    # ── 5. find_elements(pid) — no filter ──
    try:
        elems = client.find_elements(pid)
        assert isinstance(elems, list)
        print(f"  ✓ find_elements({pid}) — {len(elems)} elements")
        passed += 1
    except Exception as e:
        print(f"  ✗ find_elements({pid}) — {e}")
        failed += 1

    # ── 6. find_elements with query ──
    try:
        elems2 = client.find_elements(pid, interactive=True)
        assert isinstance(elems2, list)
        print(f"  ✓ find_elements(interactive=True) — {len(elems2)} interactive elements")
        passed += 1
    except Exception as e:
        print(f"  ✗ find_elements(interactive=True) — {e}")
        failed += 1

    # ── 7. find_elements_hwnd ──
    try:
        elems3 = client.find_elements_hwnd(hwnd, interactive=True)
        assert isinstance(elems3, list)
        print(f"  ✓ find_elements_hwnd({hwnd}) — {len(elems3)} interactive elements")
        passed += 1
    except Exception as e:
        print(f"  ✗ find_elements_hwnd({hwnd}) — {e}")
        failed += 1

    # ── 8. focus_window ──
    try:
        client.focus_window(pid)
        print(f"  ✓ focus_window({pid}) — OK")
        passed += 1
    except Exception as e:
        print(f"  ✗ focus_window({pid}) — {e}")
        failed += 1

    # ── 9. click — find a button first ──
    element_id = None
    try:
        buttons = client.find_elements(pid, element_type="Button", interactive=True)
        if buttons:
            element_id = buttons[0]["oculos_id"]
            client.click(element_id)
            print(f"  ✓ click({element_id[:8]}...) — OK")
            passed += 1
        else:
            print(f"  ⊘ click — no buttons found, skipped")
            passed += 1
    except Exception as e:
        print(f"  ✗ click — {e}")
        failed += 1

    # ── 10. focus (element) ──
    try:
        if element_id:
            # Re-fetch since tree might have changed
            buttons = client.find_elements(pid, element_type="Button", interactive=True)
            if buttons:
                eid = buttons[0]["oculos_id"]
                client.focus(eid)
                print(f"  ✓ focus({eid[:8]}...) — OK")
                passed += 1
            else:
                print(f"  ⊘ focus — no buttons, skipped")
                passed += 1
        else:
            print(f"  ⊘ focus — no element, skipped")
            passed += 1
    except Exception as e:
        print(f"  ✗ focus — {e}")
        failed += 1

    # ── 11. highlight ──
    try:
        elems_h = client.find_elements(pid, interactive=True)
        if elems_h:
            hid = elems_h[0]["oculos_id"]
            client.highlight(hid, duration_ms=500)
            print(f"  ✓ highlight({hid[:8]}...) — OK")
            passed += 1
        else:
            print(f"  ⊘ highlight — no elements, skipped")
            passed += 1
    except Exception as e:
        print(f"  ✗ highlight — {e}")
        failed += 1

    # ── 12. OculOSError handling ──
    try:
        client.click("nonexistent-id-12345")
        print(f"  ✗ error handling — should have raised OculOSError")
        failed += 1
    except OculOSError as e:
        print(f"  ✓ error handling — OculOSError raised: '{str(e)[:50]}...'")
        passed += 1
    except Exception as e:
        print(f"  ✗ error handling — unexpected: {type(e).__name__}: {e}")
        failed += 1

    # ── 13. Custom base URL ──
    try:
        bad_client = OculOS("http://127.0.0.1:9999")
        bad_client.health()
        print(f"  ✗ bad URL — should have raised")
        failed += 1
    except Exception:
        print(f"  ✓ bad URL — connection error raised correctly")
        passed += 1

    # ── Summary ──
    total = passed + failed
    print(f"\n{'='*40}")
    print(f"  Python SDK: {passed}/{total} passed")
    if failed:
        print(f"  ✗ {failed} FAILED")
    else:
        print(f"  ✓ ALL PASSED")
    print(f"{'='*40}")
    return failed == 0


if __name__ == "__main__":
    print("OculOS Python SDK Test\n")
    ok = test()
    sys.exit(0 if ok else 1)
