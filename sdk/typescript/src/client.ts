import type { Window, UiElement, HealthInfo, FindOptions, ApiResponse } from "./types";

export class OculOS {
  private baseUrl: string;

  constructor(baseUrl: string = "http://127.0.0.1:7878") {
    this.baseUrl = baseUrl.replace(/\/$/, "");
  }

  // ── Discovery ──────────────────────────────────────────────

  async listWindows(): Promise<Window[]> {
    return this.get<Window[]>("/windows");
  }

  async getTree(pid: number): Promise<UiElement> {
    return this.get<UiElement>(`/windows/${pid}/tree`);
  }

  async getTreeHwnd(hwnd: number): Promise<UiElement> {
    return this.get<UiElement>(`/hwnd/${hwnd}/tree`);
  }

  async findElements(pid: number, opts: FindOptions = {}): Promise<UiElement[]> {
    const params = new URLSearchParams();
    if (opts.query) params.set("q", opts.query);
    if (opts.type) params.set("type", opts.type);
    if (opts.interactive !== undefined) params.set("interactive", String(opts.interactive));
    const qs = params.toString();
    return this.get<UiElement[]>(`/windows/${pid}/find${qs ? `?${qs}` : ""}`);
  }

  async findElementsHwnd(hwnd: number, opts: FindOptions = {}): Promise<UiElement[]> {
    const params = new URLSearchParams();
    if (opts.query) params.set("q", opts.query);
    if (opts.type) params.set("type", opts.type);
    if (opts.interactive !== undefined) params.set("interactive", String(opts.interactive));
    const qs = params.toString();
    return this.get<UiElement[]>(`/hwnd/${hwnd}/find${qs ? `?${qs}` : ""}`);
  }

  // ── Window operations ──────────────────────────────────────

  async focusWindow(pid: number): Promise<void> {
    await this.post(`/windows/${pid}/focus`);
  }

  async closeWindow(pid: number): Promise<void> {
    await this.post(`/windows/${pid}/close`);
  }

  // ── Element interactions ───────────────────────────────────

  async click(elementId: string): Promise<void> {
    await this.post(`/interact/${elementId}/click`);
  }

  async setText(elementId: string, text: string): Promise<void> {
    await this.post(`/interact/${elementId}/set-text`, { text });
  }

  async sendKeys(elementId: string, keys: string): Promise<void> {
    await this.post(`/interact/${elementId}/send-keys`, { keys });
  }

  async focus(elementId: string): Promise<void> {
    await this.post(`/interact/${elementId}/focus`);
  }

  async toggle(elementId: string): Promise<void> {
    await this.post(`/interact/${elementId}/toggle`);
  }

  async expand(elementId: string): Promise<void> {
    await this.post(`/interact/${elementId}/expand`);
  }

  async collapse(elementId: string): Promise<void> {
    await this.post(`/interact/${elementId}/collapse`);
  }

  async select(elementId: string): Promise<void> {
    await this.post(`/interact/${elementId}/select`);
  }

  async setRange(elementId: string, value: number): Promise<void> {
    await this.post(`/interact/${elementId}/set-range`, { value });
  }

  async scroll(elementId: string, direction: string): Promise<void> {
    await this.post(`/interact/${elementId}/scroll`, { direction });
  }

  async scrollIntoView(elementId: string): Promise<void> {
    await this.post(`/interact/${elementId}/scroll-into-view`);
  }

  async highlight(elementId: string, durationMs: number = 2000): Promise<void> {
    await this.post(`/interact/${elementId}/highlight`, { duration_ms: durationMs });
  }

  // ── System ─────────────────────────────────────────────────

  async health(): Promise<HealthInfo> {
    return this.get<HealthInfo>("/health");
  }

  // ── Internals ──────────────────────────────────────────────

  private async get<T>(path: string): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`);
    if (!res.ok) throw new Error(`HTTP ${res.status}: ${res.statusText}`);
    const body: ApiResponse<T> = await res.json();
    if (!body.success) throw new OculOSError(body.error ?? "Unknown error");
    return body.data;
  }

  private async post<T = unknown>(path: string, json?: Record<string, unknown>): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method: "POST",
      headers: json ? { "Content-Type": "application/json" } : {},
      body: json ? JSON.stringify(json) : undefined,
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}: ${res.statusText}`);
    const body: ApiResponse<T> = await res.json();
    if (!body.success) throw new OculOSError(body.error ?? "Unknown error");
    return body.data;
  }
}

export class OculOSError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "OculOSError";
  }
}
