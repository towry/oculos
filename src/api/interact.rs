use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use tokio::task;

use crate::{
    api::{ws::WsEvent, AppState},
    types::{
        ApiResponse, HighlightPayload, ScrollPayload, SendKeysPayload, SetRangePayload,
        SetTextPayload,
    },
};

// ── Basic interactions ────────────────────────────────────────────────────────

/// POST /interact/:id/click
pub async fn click(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.click_element(&id2))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "click".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "click" }))))
}

/// POST /interact/:id/set-text  body: { "text": "..." }
pub async fn set_text(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<SetTextPayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.set_text(&id2, &p.text))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "set-text".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "set-text" }))))
}

/// POST /interact/:id/send-keys  body: { "keys": "Hello World" }
pub async fn send_keys(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<SendKeysPayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.send_keys(&id2, &p.keys))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "send-keys".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "send-keys" }))))
}

/// POST /interact/:id/focus
pub async fn focus(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.focus_element(&id2))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "focus".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "focus" }))))
}

// ── Pattern-specific interactions ─────────────────────────────────────────────

/// POST /interact/:id/toggle
pub async fn toggle(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.toggle_element(&id2))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "toggle".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "toggle" }))))
}

/// POST /interact/:id/expand
pub async fn expand(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.expand_element(&id2))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "expand".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "expand" }))))
}

/// POST /interact/:id/collapse
pub async fn collapse(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.collapse_element(&id2))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "collapse".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "collapse" }))))
}

/// POST /interact/:id/select
pub async fn select(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let id2 = id.clone();
    task::spawn_blocking(move || b.select_element(&id2))
        .await
        .map_err(e)?
        .map_err(e)?;
    let _ = s.ws_tx.send(WsEvent::Action {
        action: "select".into(),
        element_id: id,
        success: true,
    });
    Ok(Json(ApiResponse::ok(json!({ "action": "select" }))))
}

/// POST /interact/:id/set-range  body: { "value": 42.0 }
pub async fn set_range(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<SetRangePayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.set_range(&id, p.value))
        .await
        .map_err(e)?
        .map_err(e)?;
    Ok(Json(ApiResponse::ok(
        json!({ "action": "set-range", "value": p.value }),
    )))
}

/// POST /interact/:id/scroll  body: { "direction": "down" }
pub async fn scroll(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<ScrollPayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let dir = p.direction.clone();
    task::spawn_blocking(move || b.scroll_element(&id, &dir))
        .await
        .map_err(e)?
        .map_err(e)?;
    Ok(Json(ApiResponse::ok(
        json!({ "action": "scroll", "direction": p.direction }),
    )))
}

/// POST /interact/:id/scroll-into-view
pub async fn scroll_into_view(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.scroll_into_view(&id))
        .await
        .map_err(e)?
        .map_err(e)?;
    Ok(Json(ApiResponse::ok(
        json!({ "action": "scroll-into-view" }),
    )))
}

/// POST /interact/:id/highlight  body (optional): { "duration_ms": 2000 }
pub async fn highlight(
    State(s): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<HighlightPayload>>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let dur = body.map(|b| b.duration_ms).unwrap_or(2000);
    let b = s.backend.clone();
    let rect = task::spawn_blocking(move || b.highlight_element(&id, dur))
        .await
        .map_err(e)?
        .map_err(e)?;
    Ok(Json(ApiResponse::ok(
        json!({ "action": "highlight", "rect": { "x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height } }),
    )))
}

// ── Batch operations ──────────────────────────────────────────────────────────

/// POST /interact/batch
///
/// Execute multiple interactions in a single request.
/// Body: { "actions": [ { "element_id": "...", "action": "click" }, ... ] }
///
/// Each action object:
///   - element_id: String (required)
///   - action: "click" | "set-text" | "send-keys" | "focus" | "toggle" | "expand" | "collapse" | "select"
///   - text: String (for set-text)
///   - keys: String (for send-keys)
///   - value: f64 (for set-range)
///   - direction: String (for scroll)
#[derive(Debug, serde::Deserialize)]
pub struct BatchPayload {
    pub actions: Vec<BatchAction>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BatchAction {
    pub element_id: String,
    pub action: String,
    pub text: Option<String>,
    pub keys: Option<String>,
    pub value: Option<f64>,
    pub direction: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct BatchResult {
    pub index: usize,
    pub action: String,
    pub element_id: String,
    pub success: bool,
    pub error: Option<String>,
}

pub async fn batch(
    State(s): State<AppState>,
    Json(payload): Json<BatchPayload>,
) -> Json<ApiResponse<Vec<BatchResult>>> {
    let mut results = Vec::new();

    for (i, act) in payload.actions.iter().enumerate() {
        let b = s.backend.clone();
        let eid = act.element_id.clone();
        let action_name = act.action.clone();

        let res = match act.action.as_str() {
            "click" => {
                let id = eid.clone();
                task::spawn_blocking(move || b.click_element(&id)).await
            }
            "set-text" => {
                let id = eid.clone();
                let text = act.text.clone().unwrap_or_default();
                task::spawn_blocking(move || b.set_text(&id, &text)).await
            }
            "send-keys" => {
                let id = eid.clone();
                let keys = act.keys.clone().unwrap_or_default();
                task::spawn_blocking(move || b.send_keys(&id, &keys)).await
            }
            "focus" => {
                let id = eid.clone();
                task::spawn_blocking(move || b.focus_element(&id)).await
            }
            "toggle" => {
                let id = eid.clone();
                task::spawn_blocking(move || b.toggle_element(&id)).await
            }
            "expand" => {
                let id = eid.clone();
                task::spawn_blocking(move || b.expand_element(&id)).await
            }
            "collapse" => {
                let id = eid.clone();
                task::spawn_blocking(move || b.collapse_element(&id)).await
            }
            "select" => {
                let id = eid.clone();
                task::spawn_blocking(move || b.select_element(&id)).await
            }
            "set-range" => {
                let id = eid.clone();
                let val = act.value.unwrap_or(0.0);
                task::spawn_blocking(move || b.set_range(&id, val)).await
            }
            "scroll" => {
                let id = eid.clone();
                let dir = act.direction.clone().unwrap_or_else(|| "down".into());
                task::spawn_blocking(move || b.scroll_element(&id, &dir)).await
            }
            other => {
                results.push(BatchResult {
                    index: i,
                    action: action_name,
                    element_id: eid,
                    success: false,
                    error: Some(format!("Unknown action: {other}")),
                });
                continue;
            }
        };

        let (success, error) = match res {
            Ok(Ok(())) => (true, None),
            Ok(Err(e)) => (false, Some(e.to_string())),
            Err(e) => (false, Some(e.to_string())),
        };

        results.push(BatchResult {
            index: i,
            action: action_name,
            element_id: eid,
            success,
            error,
        });
    }

    Json(ApiResponse::ok(results))
}

// ── Helper ────────────────────────────────────────────────────────────────────

type Err = (StatusCode, Json<ApiResponse<()>>);

fn e(err: impl ToString) -> Err {
    let msg = err.to_string();
    let status = if msg.contains("not found") {
        StatusCode::NOT_FOUND
    } else if msg.contains("not supported") || msg.contains("invalid") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(ApiResponse::err(msg)))
}
