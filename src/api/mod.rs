pub mod interact;
pub mod windows;

use std::sync::Arc;
use axum::{routing::{get, post}, Router};
use crate::platform::UiBackend;

#[derive(Clone)]
pub struct AppState {
    pub backend: Arc<dyn UiBackend>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        // ── Discovery ──────────────────────────────────────────────────────
        .route("/windows",                    get(windows::list_windows))
        .route("/windows/:pid/tree",          get(windows::get_tree))
        .route("/windows/:pid/find",          get(windows::find_elements))
        // HWND-based (for apps with multiple windows sharing the same PID)
        .route("/hwnd/:hwnd/tree",            get(windows::get_tree_hwnd))
        .route("/hwnd/:hwnd/find",            get(windows::find_elements_hwnd))
        // ── Window operations ──────────────────────────────────────────────
        .route("/windows/:pid/focus",         post(windows::focus_window))
        .route("/windows/:pid/close",         post(windows::close_window))
        // ── Element interactions ───────────────────────────────────────────
        .route("/interact/:id/click",         post(interact::click))
        .route("/interact/:id/set-text",      post(interact::set_text))
        .route("/interact/:id/send-keys",     post(interact::send_keys))
        .route("/interact/:id/focus",         post(interact::focus))
        .route("/interact/:id/toggle",        post(interact::toggle))
        .route("/interact/:id/expand",        post(interact::expand))
        .route("/interact/:id/collapse",      post(interact::collapse))
        .route("/interact/:id/select",        post(interact::select))
        .route("/interact/:id/set-range",     post(interact::set_range))
        .route("/interact/:id/scroll",        post(interact::scroll))
        .route("/interact/:id/scroll-into-view", post(interact::scroll_into_view))
        // ── Health ─────────────────────────────────────────────────────────
        .route("/health",                     get(health))
        .with_state(state)
}

async fn health() -> &'static str { "OculOS is running" }
