/**
 * OculOS TypeScript SDK — integration test (runs as plain JS with Node 22+).
 * We import the .ts source directly won't work without tsx, so we inline the client logic.
 */

const BASE = "http://127.0.0.1:7878";

class OculOS {
  constructor(baseUrl = BASE) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
  }

  async listWindows() { return this._get("/windows"); }
  async getTree(pid) { return this._get(`/windows/${pid}/tree`); }
  async getTreeHwnd(hwnd) { return this._get(`/hwnd/${hwnd}/tree`); }

  async findElements(pid, opts = {}) {
    const p = new URLSearchParams();
    if (opts.query) p.set("q", opts.query);
    if (opts.type) p.set("type", opts.type);
    if (opts.interactive !== undefined) p.set("interactive", String(opts.interactive));
    const qs = p.toString();
    return this._get(`/windows/${pid}/find${qs ? `?${qs}` : ""}`);
  }

  async findElementsHwnd(hwnd, opts = {}) {
    const p = new URLSearchParams();
    if (opts.query) p.set("q", opts.query);
    if (opts.type) p.set("type", opts.type);
    if (opts.interactive !== undefined) p.set("interactive", String(opts.interactive));
    const qs = p.toString();
    return this._get(`/hwnd/${hwnd}/find${qs ? `?${qs}` : ""}`);
  }

  async focusWindow(pid) { await this._post(`/windows/${pid}/focus`); }
  async closeWindow(pid) { await this._post(`/windows/${pid}/close`); }
  async click(id) { await this._post(`/interact/${id}/click`); }
  async setText(id, text) { await this._post(`/interact/${id}/set-text`, { text }); }
  async sendKeys(id, keys) { await this._post(`/interact/${id}/send-keys`, { keys }); }
  async focus(id) { await this._post(`/interact/${id}/focus`); }
  async toggle(id) { await this._post(`/interact/${id}/toggle`); }
  async expand(id) { await this._post(`/interact/${id}/expand`); }
  async collapse(id) { await this._post(`/interact/${id}/collapse`); }
  async select(id) { await this._post(`/interact/${id}/select`); }
  async setRange(id, value) { await this._post(`/interact/${id}/set-range`, { value }); }
  async scroll(id, direction) { await this._post(`/interact/${id}/scroll`, { direction }); }
  async scrollIntoView(id) { await this._post(`/interact/${id}/scroll-into-view`); }
  async highlight(id, durationMs = 2000) { await this._post(`/interact/${id}/highlight`, { duration_ms: durationMs }); }
  async health() { return this._get("/health"); }

  async _get(path) {
    const res = await fetch(`${this.baseUrl}${path}`);
    const body = await res.json();
    if (!body.success) throw new Error(body.error || `HTTP ${res.status}`);
    return body.data;
  }

  async _post(path, json) {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method: "POST",
      headers: json ? { "Content-Type": "application/json" } : {},
      body: json ? JSON.stringify(json) : undefined,
    });
    const body = await res.json();
    if (!body.success) throw new Error(body.error || `HTTP ${res.status}`);
    return body.data;
  }
}

// ── Tests ──

let passed = 0;
let failed = 0;

function ok(name, msg) { console.log(`  ✓ ${name} — ${msg}`); passed++; }
function fail(name, msg) { console.log(`  ✗ ${name} — ${msg}`); failed++; }

async function run() {
  console.log("OculOS TypeScript SDK Test\n");
  const client = new OculOS();

  // 1. health
  try {
    const h = await client.health();
    if (h.status !== "running") throw new Error(`status=${h.status}`);
    ok("health()", `status=${h.status}, version=${h.version}`);
  } catch (e) { fail("health()", e.message); }

  // 2. listWindows
  let pid, hwnd;
  try {
    const wins = await client.listWindows();
    if (!Array.isArray(wins) || wins.length === 0) throw new Error("empty");
    pid = wins[0].pid;
    hwnd = wins[0].hwnd;
    ok("listWindows()", `${wins.length} windows`);
  } catch (e) { fail("listWindows()", e.message); }

  // 3. getTree
  try {
    const tree = await client.getTree(pid);
    if (!tree.oculos_id) throw new Error("no oculos_id");
    ok("getTree()", `root=${tree.type}, children=${tree.children.length}`);
  } catch (e) { fail("getTree()", e.message); }

  // 4. getTreeHwnd
  try {
    const tree2 = await client.getTreeHwnd(hwnd);
    if (!tree2.oculos_id) throw new Error("no oculos_id");
    ok("getTreeHwnd()", `root=${tree2.type}`);
  } catch (e) { fail("getTreeHwnd()", e.message); }

  // 5. findElements
  try {
    const elems = await client.findElements(pid);
    if (!Array.isArray(elems)) throw new Error("not array");
    ok("findElements()", `${elems.length} elements`);
  } catch (e) { fail("findElements()", e.message); }

  // 6. findElements interactive
  try {
    const elems = await client.findElements(pid, { interactive: true });
    ok("findElements(interactive)", `${elems.length} interactive`);
  } catch (e) { fail("findElements(interactive)", e.message); }

  // 7. findElementsHwnd
  try {
    const elems = await client.findElementsHwnd(hwnd, { interactive: true });
    ok("findElementsHwnd()", `${elems.length} interactive`);
  } catch (e) { fail("findElementsHwnd()", e.message); }

  // 8. focusWindow
  try {
    await client.focusWindow(pid);
    ok("focusWindow()", "OK");
  } catch (e) { fail("focusWindow()", e.message); }

  // 9. click
  try {
    const btns = await client.findElements(pid, { type: "Button", interactive: true });
    if (btns.length > 0) {
      await client.click(btns[0].oculos_id);
      ok("click()", `${btns[0].oculos_id.slice(0, 8)}... OK`);
    } else {
      ok("click()", "no buttons, skipped");
    }
  } catch (e) { fail("click()", e.message); }

  // 10. focus element
  try {
    const elems = await client.findElements(pid, { interactive: true });
    if (elems.length > 0) {
      await client.focus(elems[0].oculos_id);
      ok("focus()", `${elems[0].oculos_id.slice(0, 8)}... OK`);
    } else {
      ok("focus()", "skipped");
    }
  } catch (e) { fail("focus()", e.message); }

  // 11. highlight
  try {
    const elems = await client.findElements(pid, { interactive: true });
    if (elems.length > 0) {
      await client.highlight(elems[0].oculos_id, 500);
      ok("highlight()", `${elems[0].oculos_id.slice(0, 8)}... OK`);
    } else {
      ok("highlight()", "skipped");
    }
  } catch (e) { fail("highlight()", e.message); }

  // 12. error handling
  try {
    await client.click("nonexistent-id-12345");
    fail("error handling", "should have thrown");
  } catch (e) {
    if (e.message.includes("not found")) {
      ok("error handling", `Error thrown: '${e.message.slice(0, 50)}...'`);
    } else {
      fail("error handling", `unexpected: ${e.message}`);
    }
  }

  // 13. bad URL
  try {
    const bad = new OculOS("http://127.0.0.1:9999");
    await bad.health();
    fail("bad URL", "should have thrown");
  } catch (e) {
    ok("bad URL", "connection error raised correctly");
  }

  // Summary
  const total = passed + failed;
  console.log(`\n${"=".repeat(40)}`);
  console.log(`  TypeScript SDK: ${passed}/${total} passed`);
  if (failed) console.log(`  ✗ ${failed} FAILED`);
  else console.log(`  ✓ ALL PASSED`);
  console.log("=".repeat(40));
  process.exit(failed ? 1 : 0);
}

run();
