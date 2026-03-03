use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use tokio::task;

use crate::{
    api::AppState,
    types::{ApiResponse, ElementType, UiElement, WindowInfo},
};

// ── Discovery endpoints ───────────────────────────────────────────────────────

/// GET /windows
pub async fn list_windows(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<WindowInfo>>>, Err> {
    let b = state.backend.clone();
    let r = task::spawn_blocking(move || b.list_windows()).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(r)))
}

/// GET /hwnd/:hwnd/tree
pub async fn get_tree_hwnd(
    State(state): State<AppState>,
    Path(hwnd): Path<usize>,
) -> Result<Json<ApiResponse<UiElement>>, Err> {
    let b = state.backend.clone();
    let r = task::spawn_blocking(move || b.get_ui_tree_hwnd(hwnd)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(r)))
}

/// GET /hwnd/:hwnd/find
pub async fn find_elements_hwnd(
    State(state): State<AppState>,
    Path(hwnd): Path<usize>,
    Query(params): Query<FindQuery>,
) -> Result<Json<ApiResponse<Vec<UiElement>>>, Err> {
    let et: Option<ElementType> = params.element_type.as_deref().map(ElementType::from);
    let interactive_only = params.interactive.as_deref() == Some("true");
    let query = params.q.clone();
    let b = state.backend.clone();
    let r = task::spawn_blocking(move || {
        b.find_elements_hwnd(hwnd, query.as_deref(), et.as_ref(), interactive_only)
    })
    .await
    .map_err(e)?
    .map_err(e)?;
    Ok(Json(ApiResponse::ok(r)))
}

/// GET /windows/:pid/tree
pub async fn get_tree(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Result<Json<ApiResponse<UiElement>>, Err> {
    let b = state.backend.clone();
    let r = task::spawn_blocking(move || b.get_ui_tree(pid)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(r)))
}

/// GET /windows/:pid/find
///
/// Query parameters:
///   q             — Text to search in label or automation_id (optional)
///   type          — Element type filter, e.g. "Button" (optional)
///   interactive   — If "true", return only elements with at least one action (optional)
#[derive(Deserialize)]
pub struct FindQuery {
    pub q: Option<String>,
    #[serde(rename = "type")]
    pub element_type: Option<String>,
    pub interactive: Option<String>,
}

pub async fn find_elements(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
    Query(params): Query<FindQuery>,
) -> Result<Json<ApiResponse<Vec<UiElement>>>, Err> {
    let et: Option<ElementType> = params.element_type.as_deref().map(ElementType::from);
    let interactive_only = params.interactive.as_deref() == Some("true");
    let query = params.q.clone();

    let b = state.backend.clone();
    let r = task::spawn_blocking(move || {
        b.find_elements(pid, query.as_deref(), et.as_ref(), interactive_only)
    })
    .await
    .map_err(e)?
    .map_err(e)?;

    Ok(Json(ApiResponse::ok(r)))
}

// ── Window operation endpoints ────────────────────────────────────────────────

/// POST /windows/:pid/focus
pub async fn focus_window(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Result<Json<ApiResponse<()>>, Err> {
    let b = state.backend.clone();
    task::spawn_blocking(move || b.focus_window(pid)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(())))
}

/// POST /windows/:pid/close
pub async fn close_window(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Result<Json<ApiResponse<()>>, Err> {
    let b = state.backend.clone();
    task::spawn_blocking(move || b.close_window(pid)).await.map_err(e)?.map_err(e)?;
    Ok(Json(ApiResponse::ok(())))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

type Err = (StatusCode, Json<ApiResponse<()>>);

fn e(err: impl ToString) -> Err {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::err(err.to_string())))
}

