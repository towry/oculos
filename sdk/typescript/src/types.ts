export interface Window {
  pid: number;
  hwnd: number;
  title: string;
  exe_name: string;
  rect: Rect;
  visible: boolean;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface UiElement {
  oculos_id: string;
  type: string;
  label: string;
  value: string | null;
  enabled: boolean;
  focused: boolean;
  actions: string[];
  toggle_state: string | null;
  is_selected: boolean | null;
  expand_state: string | null;
  range: Range | null;
  automation_id: string | null;
  help_text: string | null;
  rect: Rect;
  children: UiElement[];
}

export interface Range {
  value: number;
  minimum: number;
  maximum: number;
  step: number;
}

export interface HealthInfo {
  status: string;
  version: string;
  uptime_seconds: number;
  platform: string;
}

export interface FindOptions {
  query?: string;
  type?: string;
  interactive?: boolean;
}

export interface ApiResponse<T> {
  success: boolean;
  data: T;
  error: string | null;
}
