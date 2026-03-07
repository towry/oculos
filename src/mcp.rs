//! MCP (Model Context Protocol) server over stdin/stdout.
//!
//! Launch OculOS with `--mcp` to run as an MCP server instead of an HTTP server.
//! Compatible with Claude Code, Claude Desktop, Cursor, Windsurf, and any other
//! MCP-compatible AI agent host.
//!
//! Protocol: JSON-RPC 2.0, newline-delimited, over stdin/stdout.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::platform::UiBackend;
use crate::types::ElementType;

// ── JSON-RPC 2.0 types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    #[serde(default)]
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

fn rpc_ok(id: Value, result: Value) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn rpc_err(id: Value, code: i32, msg: impl Into<String>) -> RpcResponse {
    RpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(RpcError {
            code,
            message: msg.into(),
        }),
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Runs the MCP server loop synchronously (blocks the calling thread).
/// Call from `tokio::task::spawn_blocking`.
pub fn run_mcp(backend: Arc<dyn UiBackend>) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let resp = match serde_json::from_str::<RpcRequest>(&line) {
            Err(e) => rpc_err(Value::Null, -32700, format!("Parse error: {e}")),
            Ok(req) => {
                let id = req.id.clone();
                match dispatch(&backend, req) {
                    Ok(r) => r,
                    Err(e) => rpc_err(id, -32603, e.to_string()),
                }
            }
        };

        let mut encoded = serde_json::to_string(&resp)?;
        encoded.push('\n');
        stdout.write_all(encoded.as_bytes())?;
        stdout.flush()?;
    }

    Ok(())
}

// ── Method dispatcher ─────────────────────────────────────────────────────────

fn dispatch(backend: &Arc<dyn UiBackend>, req: RpcRequest) -> Result<RpcResponse> {
    let id = req.id.clone();
    let p = &req.params;

    // Notifications (no id, no response needed — but clients may send them)
    if matches!(
        req.method.as_str(),
        "notifications/initialized" | "notifications/cancelled"
    ) {
        return Ok(rpc_ok(Value::Null, Value::Null));
    }

    let result: Value = match req.method.as_str() {
        // ── MCP lifecycle ─────────────────────────────────────────────────────
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "oculos",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),

        // ── Tool catalogue ────────────────────────────────────────────────────
        "tools/list" => json!({ "tools": tools_schema() }),

        // ── Tool invocation ───────────────────────────────────────────────────
        "tools/call" => {
            let name = p["name"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("missing 'name' in tools/call params"))?;
            let args = &p["arguments"];
            let text = call_tool(backend, name, args)?;
            json!({ "content": [{ "type": "text", "text": text }] })
        }

        other => return Ok(rpc_err(id, -32601, format!("Unknown method: {other}"))),
    };

    Ok(rpc_ok(id, result))
}

// ── Tool implementations ──────────────────────────────────────────────────────

fn call_tool(backend: &Arc<dyn UiBackend>, name: &str, args: &Value) -> Result<String> {
    let v: Value = match name {
        // ── Discovery ─────────────────────────────────────────────────────────
        "list_windows" => serde_json::to_value(backend.list_windows()?)?,

        "get_ui_tree" => serde_json::to_value(backend.get_ui_tree(need_pid(args)?)?)?,

        "get_ui_tree_hwnd" => serde_json::to_value(backend.get_ui_tree_hwnd(need_hwnd(args)?)?)?,

        "find_elements" => {
            let q = str_opt(args, "query");
            let et = et_opt(args);
            let i = args["interactive_only"].as_bool().unwrap_or(false);
            serde_json::to_value(backend.find_elements(
                need_pid(args)?,
                q.as_deref(),
                et.as_ref(),
                i,
            )?)?
        }

        "find_elements_hwnd" => {
            let q = str_opt(args, "query");
            let et = et_opt(args);
            let i = args["interactive_only"].as_bool().unwrap_or(false);
            serde_json::to_value(backend.find_elements_hwnd(
                need_hwnd(args)?,
                q.as_deref(),
                et.as_ref(),
                i,
            )?)?
        }

        // ── Element actions ───────────────────────────────────────────────────
        "click_element" => {
            backend.click_element(&need_id(args)?)?;
            json!("ok")
        }
        "focus_element" => {
            backend.focus_element(&need_id(args)?)?;
            json!("ok")
        }
        "toggle_element" => {
            backend.toggle_element(&need_id(args)?)?;
            json!("ok")
        }
        "expand_element" => {
            backend.expand_element(&need_id(args)?)?;
            json!("ok")
        }
        "collapse_element" => {
            backend.collapse_element(&need_id(args)?)?;
            json!("ok")
        }
        "select_element" => {
            backend.select_element(&need_id(args)?)?;
            json!("ok")
        }
        "scroll_into_view" => {
            backend.scroll_into_view(&need_id(args)?)?;
            json!("ok")
        }

        "set_text" => {
            let text = need_str(args, "text")?;
            backend.set_text(&need_id(args)?, &text)?;
            json!("ok")
        }

        "send_keys" => {
            let keys = need_str(args, "keys")?;
            backend.send_keys(&need_id(args)?, &keys)?;
            json!("ok")
        }

        "set_range" => {
            let value = args["value"]
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("missing 'value'"))?;
            backend.set_range(&need_id(args)?, value)?;
            json!("ok")
        }

        "scroll_element" => {
            let dir = args["direction"].as_str().unwrap_or("down").to_string();
            backend.scroll_element(&need_id(args)?, &dir)?;
            json!("ok")
        }

        // ── Window operations ─────────────────────────────────────────────────
        "focus_window" => {
            backend.focus_window(need_pid(args)?)?;
            json!("ok")
        }
        "close_window" => {
            backend.close_window(need_pid(args)?)?;
            json!("ok")
        }

        other => return Err(anyhow::anyhow!("Unknown tool: {other}")),
    };

    Ok(serde_json::to_string_pretty(&v)?)
}

// ── Argument helpers ──────────────────────────────────────────────────────────

fn need_id(args: &Value) -> Result<String> {
    args["id"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("missing required argument 'id'"))
}

fn need_pid(args: &Value) -> Result<u32> {
    args["pid"]
        .as_u64()
        .map(|n| n as u32)
        .ok_or_else(|| anyhow::anyhow!("missing required argument 'pid'"))
}

fn need_hwnd(args: &Value) -> Result<usize> {
    args["hwnd"]
        .as_u64()
        .map(|n| n as usize)
        .ok_or_else(|| anyhow::anyhow!("missing required argument 'hwnd'"))
}

fn need_str(args: &Value, key: &str) -> Result<String> {
    args[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("missing required argument '{key}'"))
}

fn str_opt(args: &Value, key: &str) -> Option<String> {
    args[key].as_str().map(String::from)
}

fn et_opt(args: &Value) -> Option<ElementType> {
    args["element_type"].as_str().map(ElementType::from)
}

// ── Tool schema definitions ───────────────────────────────────────────────────

fn tools_schema() -> Value {
    json!([
        {
            "name": "list_windows",
            "description": "List all visible top-level windows on the desktop. Returns pid, hwnd, title, exe_name, and rect for each window. Call this first to discover which application to control.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        },
        {
            "name": "get_ui_tree",
            "description": "Get the full UI element tree for a process by PID. Each element includes an oculos_id, type, label, value, state, bounding rect, and — crucially — an 'actions' list that tells you exactly what you can do with it. Elements without actions are display-only.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": {
                        "type": "integer",
                        "description": "Process ID from list_windows"
                    }
                },
                "required": ["pid"]
            }
        },
        {
            "name": "get_ui_tree_hwnd",
            "description": "Get the UI element tree for a specific window by its HWND (window handle). Use this instead of get_ui_tree when a process has multiple windows sharing the same PID — for example Teams has a meeting window and a chat window under one PID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "hwnd": {
                        "type": "integer",
                        "description": "Window handle from list_windows"
                    }
                },
                "required": ["hwnd"]
            }
        },
        {
            "name": "find_elements",
            "description": "Search for UI elements in a process by label text, automation ID, or element type. Much faster than parsing the full tree. Use interactive_only=true to find only clickable/typeable elements.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": {
                        "type": "integer",
                        "description": "Process ID"
                    },
                    "query": {
                        "type": "string",
                        "description": "Text to match against label or automation_id (case-insensitive substring match)"
                    },
                    "element_type": {
                        "type": "string",
                        "description": "Filter by element type. One of: Button, Edit, Text, CheckBox, RadioButton, ComboBox, ListBox, ListItem, TreeItem, Menu, MenuItem, TabItem, ToolBar, StatusBar, ScrollBar, Slider, ProgressBar, Image, Link, Group, Pane, Dialog, Document, DataGrid, DataItem, Table, Window, Custom"
                    },
                    "interactive_only": {
                        "type": "boolean",
                        "description": "If true, return only elements that have at least one available action"
                    }
                },
                "required": ["pid"]
            }
        },
        {
            "name": "find_elements_hwnd",
            "description": "Search for UI elements in a specific window by HWND. Same as find_elements but targets a single window handle instead of all windows for a PID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "hwnd": {
                        "type": "integer",
                        "description": "Window handle from list_windows"
                    },
                    "query": {
                        "type": "string",
                        "description": "Text to match against label or automation_id"
                    },
                    "element_type": {
                        "type": "string",
                        "description": "Filter by element type"
                    },
                    "interactive_only": {
                        "type": "boolean",
                        "description": "Only return actionable elements"
                    }
                },
                "required": ["hwnd"]
            }
        },
        {
            "name": "click_element",
            "description": "Click a UI element such as a button, link, or menu item. Uses native UI Automation invoke — no mouse movement or coordinates required. Only call this if 'click' appears in the element's actions list.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The oculos_id of the element to click"
                    }
                },
                "required": ["id"]
            }
        },
        {
            "name": "set_text",
            "description": "Set the full text content of an input field (Edit, ComboBox) by replacing its current value. Faster and more reliable than send_keys for setting text. Only call this if 'set-text' appears in the element's actions list.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id":   { "type": "string", "description": "oculos_id of the Edit element" },
                    "text": { "type": "string", "description": "New text to set" }
                },
                "required": ["id", "text"]
            }
        },
        {
            "name": "send_keys",
            "description": "Simulate keyboard input on an element character by character. Use for applications that don't support set_text, or when you need special keys. Supports: regular characters, and {KEYNAME} syntax for special keys. Special keys: {ENTER}, {TAB}, {ESC}, {SPACE}, {CTRL+A}, {CTRL+C}, {CTRL+V}, {CTRL+X}, {CTRL+Z}, {CTRL+Y}, {ALT+F4}, {WIN+D}, {F1}–{F12}, {UP}, {DOWN}, {LEFT}, {RIGHT}, {DELETE}, {BACKSPACE}, {HOME}, {END}, {PGUP}, {PGDN}. Example: 'hello world{ENTER}' or '{CTRL+A}new text{ENTER}'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id":   { "type": "string", "description": "oculos_id of the target element" },
                    "keys": { "type": "string", "description": "Characters and key sequences to send" }
                },
                "required": ["id", "keys"]
            }
        },
        {
            "name": "focus_element",
            "description": "Move keyboard focus to a UI element without activating it. Useful before sending keyboard shortcuts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "oculos_id of the element" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "toggle_element",
            "description": "Toggle a CheckBox, ToggleButton, or switch between On and Off states. Check toggle_state field first to know the current state.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "oculos_id of the CheckBox or ToggleButton" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "expand_element",
            "description": "Expand a ComboBox, TreeItem, or MenuItem to reveal its children. Check expand_state field — only call if state is 'Collapsed'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "oculos_id of the element to expand" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "collapse_element",
            "description": "Collapse an expanded ComboBox, TreeItem, or MenuItem.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "oculos_id of the element to collapse" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "select_element",
            "description": "Select a ListItem, RadioButton, or TabItem. Use this instead of click for selection-based controls.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "oculos_id of the item to select" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "set_range",
            "description": "Set a numeric value on a Slider, Spinner, or ProgressBar. Check the element's range field for minimum, maximum, and step values before calling.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id":    { "type": "string", "description": "oculos_id of the range element" },
                    "value": { "type": "number", "description": "Target numeric value (must be within min/max)" }
                },
                "required": ["id", "value"]
            }
        },
        {
            "name": "scroll_element",
            "description": "Scroll a scrollable container in the given direction.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "oculos_id of the scrollable element"
                    },
                    "direction": {
                        "type": "string",
                        "description": "Scroll direction",
                        "enum": ["up", "down", "left", "right", "page-up", "page-down"]
                    }
                },
                "required": ["id", "direction"]
            }
        },
        {
            "name": "scroll_into_view",
            "description": "Scroll an element into the visible viewport so it can be interacted with.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "oculos_id of the element to bring into view" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "focus_window",
            "description": "Bring a window to the foreground and give it keyboard focus. Call this before interacting with an application that may be in the background.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "Process ID from list_windows" }
                },
                "required": ["pid"]
            }
        },
        {
            "name": "close_window",
            "description": "Close a window gracefully (equivalent to pressing Alt+F4 or clicking the X button).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "Process ID from list_windows" }
                },
                "required": ["pid"]
            }
        }
    ])
}
