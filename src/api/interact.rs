use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use tokio::task;

use crate::{
    api::AppState,
    types::{ApiResponse, ScrollPayload, SendKeysPayload, SetRangePayload, SetTextPayload},
};

// ── Basic interactions ────────────────────────────────────────────────────────

/// POST /interact/:id/click
pub async fn click(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.click_element(&id)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "click" }))))
}

/// POST /interact/:id/set-text  body: { "text": "..." }
pub async fn set_text(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<SetTextPayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.set_text(&id, &p.text)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "set-text" }))))
}

/// POST /interact/:id/send-keys  body: { "keys": "Hello World" }
pub async fn send_keys(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<SendKeysPayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.send_keys(&id, &p.keys)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "send-keys" }))))
}

/// POST /interact/:id/focus
pub async fn focus(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.focus_element(&id)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "focus" }))))
}

// ── Pattern-specific interactions ─────────────────────────────────────────────

/// POST /interact/:id/toggle
pub async fn toggle(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.toggle_element(&id)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "toggle" }))))
}

/// POST /interact/:id/expand
pub async fn expand(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.expand_element(&id)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "expand" }))))
}

/// POST /interact/:id/collapse
pub async fn collapse(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.collapse_element(&id)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "collapse" }))))
}

/// POST /interact/:id/select
pub async fn select(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.select_element(&id)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "select" }))))
}

/// POST /interact/:id/set-range  body: { "value": 42.0 }
pub async fn set_range(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<SetRangePayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.set_range(&id, p.value)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "set-range", "value": p.value }))))
}

/// POST /interact/:id/scroll  body: { "direction": "down" }
pub async fn scroll(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<ScrollPayload>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    let dir = p.direction.clone();
    task::spawn_blocking(move || b.scroll_element(&id, &dir)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "scroll", "direction": p.direction }))))
}

/// POST /interact/:id/scroll-into-view
pub async fn scroll_into_view(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, Err> {
    let b = s.backend.clone();
    task::spawn_blocking(move || b.scroll_into_view(&id)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(json!({ "action": "scroll-into-view" }))))
}

// ── Helper ────────────────────────────────────────────────────────────────────

type Err = (StatusCode, Json<ApiResponse<()>>);

fn e(err: impl ToString) -> Err {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(err.to_string())))
}
