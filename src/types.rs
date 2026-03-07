use serde::{Deserialize, Serialize};

// ── Geometry ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

// ── Window ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub pid: u32,
    pub hwnd: usize,
    pub title: String,
    pub exe_name: String,
    pub rect: Rect,
    pub visible: bool,
}

// ── Element type ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ElementType {
    Window,
    Button,
    Edit,
    Text,
    CheckBox,
    RadioButton,
    ComboBox,
    ListBox,
    ListItem,
    TreeView,
    TreeItem,
    Menu,
    MenuItem,
    TabControl,
    TabItem,
    ToolBar,
    StatusBar,
    ScrollBar,
    Slider,
    ProgressBar,
    Image,
    Link,
    Group,
    Pane,
    Dialog,
    Document,
    DataGrid,
    DataItem,
    HeaderItem,
    Table,
    Custom,
    Unknown,
}

// ── State types ───────────────────────────────────────────────────────────────

/// Toggle state for CheckBoxes, ToggleButtons, etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToggleState {
    Off,
    On,
    Indeterminate,
}

/// Expand/Collapse state for ComboBoxes, TreeItems, etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExpandState {
    Collapsed,
    Expanded,
    PartiallyExpanded,
    LeafNode,
}

/// Range info for Sliders, Spinners, ProgressBars.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeInfo {
    pub value: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub read_only: bool,
}

// ── UI Element (the Virtual DOM node) ────────────────────────────────────────

/// A single node in the UI element tree.
///
/// The `actions` field is the key for AI agents — it explicitly lists every
/// operation that can be performed on this element via the interact API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiElement {
    /// Session-scoped unique ID. Use this for all /interact calls.
    pub oculos_id: String,

    /// Semantic element type.
    #[serde(rename = "type")]
    pub element_type: ElementType,

    /// Accessible name / label (what a screen reader would announce).
    pub label: String,

    /// Current text/value (Edit content, selected ComboBox item, etc.)
    pub value: Option<String>,

    /// Full text content for Document/RichText elements.
    pub text_content: Option<String>,

    /// Bounding box in screen coordinates.
    pub rect: Rect,

    // ── State ──────────────────────────────────────────────────────────────
    pub enabled: bool,
    pub focused: bool,
    pub is_keyboard_focusable: bool,

    /// For CheckBox, ToggleButton — "On" / "Off" / "Indeterminate"
    pub toggle_state: Option<ToggleState>,

    /// For ListItem, RadioButton, TabItem — is this currently selected?
    pub is_selected: Option<bool>,

    /// For ComboBox, TreeItem, MenuItem — expanded or collapsed?
    pub expand_state: Option<ExpandState>,

    /// For Slider, Spinner, ProgressBar — numeric range info.
    pub range: Option<RangeInfo>,

    // ── Metadata ───────────────────────────────────────────────────────────
    /// Developer-assigned automation ID (stable across runs).
    pub automation_id: Option<String>,
    pub class_name: Option<String>,
    pub help_text: Option<String>,
    pub keyboard_shortcut: Option<String>,

    // ── The key for AI agents ──────────────────────────────────────────────
    /// Explicit list of actions available on this element.
    ///
    /// Possible values:
    ///   "click"            → POST /interact/{id}/click
    ///   "set-text"         → POST /interact/{id}/set-text
    ///   "send-keys"        → POST /interact/{id}/send-keys
    ///   "toggle"           → POST /interact/{id}/toggle
    ///   "expand"           → POST /interact/{id}/expand
    ///   "collapse"         → POST /interact/{id}/collapse
    ///   "select"           → POST /interact/{id}/select
    ///   "set-range"        → POST /interact/{id}/set-range
    ///   "scroll-into-view" → POST /interact/{id}/scroll-into-view
    ///   "focus"            → POST /interact/{id}/focus
    pub actions: Vec<String>,

    /// Child elements.
    pub children: Vec<UiElement>,
}

// ── Request payloads ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SetTextPayload {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct SendKeysPayload {
    /// Text to type into the focused element character by character.
    pub keys: String,
}

#[derive(Debug, Deserialize)]
pub struct SetRangePayload {
    pub value: f64,
}

#[derive(Debug, Deserialize)]
pub struct ScrollPayload {
    /// "up" | "down" | "left" | "right" | "page-up" | "page-down"
    pub direction: String,
}

#[derive(Debug, Deserialize)]
pub struct HighlightPayload {
    #[serde(default = "default_highlight_duration")]
    pub duration_ms: u64,
}

fn default_highlight_duration() -> u64 {
    2000
}

// ── ElementType helpers ───────────────────────────────────────────────────────

impl From<&str> for ElementType {
    fn from(s: &str) -> Self {
        match s {
            "Button" => ElementType::Button,
            "Edit" => ElementType::Edit,
            "Text" => ElementType::Text,
            "CheckBox" => ElementType::CheckBox,
            "RadioButton" => ElementType::RadioButton,
            "ComboBox" => ElementType::ComboBox,
            "ListBox" => ElementType::ListBox,
            "ListItem" => ElementType::ListItem,
            "TreeItem" => ElementType::TreeItem,
            "Menu" => ElementType::Menu,
            "MenuItem" => ElementType::MenuItem,
            "TabItem" => ElementType::TabItem,
            "ToolBar" => ElementType::ToolBar,
            "StatusBar" => ElementType::StatusBar,
            "ScrollBar" => ElementType::ScrollBar,
            "Slider" => ElementType::Slider,
            "ProgressBar" => ElementType::ProgressBar,
            "Image" => ElementType::Image,
            "Link" => ElementType::Link,
            "Group" => ElementType::Group,
            "Pane" => ElementType::Pane,
            "Dialog" => ElementType::Dialog,
            "Document" => ElementType::Document,
            "DataGrid" => ElementType::DataGrid,
            "DataItem" => ElementType::DataItem,
            "Table" => ElementType::Table,
            "Window" => ElementType::Window,
            "Custom" => ElementType::Custom,
            _ => ElementType::Unknown,
        }
    }
}

// ── Generic response wrapper ──────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
}

impl ApiResponse<()> {
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}
