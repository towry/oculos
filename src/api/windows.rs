use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
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
    let r = task::spawn_blocking(move || b.list_windows())
        .await
        .map_err(e)?
        .map_err(e)?;
    Ok(Json(ApiResponse::ok(r)))
}

/// GET /hwnd/:hwnd/tree
pub async fn get_tree_hwnd(
    State(state): State<AppState>,
    Path(hwnd): Path<usize>,
) -> Result<Json<ApiResponse<UiElement>>, Err> {
    let b = state.backend.clone();
    let r = task::spawn_blocking(move || b.get_ui_tree_hwnd(hwnd))
        .await
        .map_err(e)?
        .map_err(e)?;
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
    let r = task::spawn_blocking(move || b.get_ui_tree(pid))
        .await
        .map_err(e)?
        .map_err(e)?;
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
    task::spawn_blocking(move || b.focus_window(pid))
        .await
        .map_err(e)?
        .map_err(e)?;
    Ok(Json(ApiResponse::ok(())))
}

/// POST /windows/:pid/close
pub async fn close_window(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Result<Json<ApiResponse<()>>, Err> {
    let b = state.backend.clone();
    task::spawn_blocking(move || b.close_window(pid))
        .await
        .map_err(e)?
        .map_err(e)?;
    Ok(Json(ApiResponse::ok(())))
}

// ── Screenshot ───────────────────────────────────────────────────────────────

/// GET /windows/:pid/screenshot — returns PNG image
pub async fn screenshot_window(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
) -> Result<impl IntoResponse, Err> {
    let b = state.backend.clone();
    let png = task::spawn_blocking(move || b.screenshot_window(pid))
        .await
        .map_err(e)?
        .map_err(e)?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        png,
    ))
}

// ── Wait / Poll ──────────────────────────────────────────────────────────────

/// GET /windows/:pid/wait?q=Submit&type=Button&timeout=5000
///
/// Polls every 250ms until at least one matching element appears, or until
/// timeout (ms) is reached. Default timeout: 5000ms. Max: 30000ms.
#[derive(Deserialize)]
pub struct WaitQuery {
    pub q: Option<String>,
    #[serde(rename = "type")]
    pub element_type: Option<String>,
    pub interactive: Option<String>,
    /// Timeout in milliseconds (default 5000, max 30000).
    pub timeout: Option<u64>,
}

pub async fn wait_for_element(
    State(state): State<AppState>,
    Path(pid): Path<u32>,
    Query(params): Query<WaitQuery>,
) -> Result<Json<ApiResponse<Vec<UiElement>>>, Err> {
    let timeout_ms = params.timeout.unwrap_or(5000).min(30000);
    let et: Option<ElementType> = params.element_type.as_deref().map(ElementType::from);
    let interactive_only = params.interactive.as_deref() == Some("true");
    let query = params.q.clone();

    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);

    loop {
        let b = state.backend.clone();
        let q = query.clone();
        let et2 = et.clone();
        let result = task::spawn_blocking(move || {
            b.find_elements(pid, q.as_deref(), et2.as_ref(), interactive_only)
        })
        .await
        .map_err(e)?
        .map_err(e)?;

        if !result.is_empty() {
            return Ok(Json(ApiResponse::ok(result)));
        }

        if std::time::Instant::now() >= deadline {
            return Err((
                StatusCode::REQUEST_TIMEOUT,
                Json(ApiResponse::err(format!(
                    "No matching element found within {}ms",
                    timeout_ms
                ))),
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
