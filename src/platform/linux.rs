use anyhow::{anyhow, Context, Result};
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

use atspi::{
    proxy::accessible::AccessibleProxy,
    proxy::action::ActionProxy,
    proxy::component::ComponentProxy,
    proxy::editable_text::EditableTextProxy,
    proxy::text::TextProxy,
    proxy::value::ValueProxy,
    AccessibilityConnection, Role,
};
use zbus::Connection;

use crate::{
    platform::UiBackend,
    types::{ElementType, ExpandState, RangeInfo, Rect, ToggleState, UiElement, WindowInfo},
};

// ── Element registry ──────────────────────────────────────────────────────────

struct SafeElement {
    bus_name: String,
    object_path: String,
}
unsafe impl Send for SafeElement {}
unsafe impl Sync for SafeElement {}

type IdRegistry = Arc<DashMap<String, SafeElement>>;

// ── Backend ───────────────────────────────────────────────────────────────────

pub struct LinuxUiBackend {
    connection: Connection,
    registry: IdRegistry,
    rt: tokio::runtime::Handle,
}

impl LinuxUiBackend {
    pub fn new() -> Result<Self> {
        // We need an async runtime handle to run AT-SPI calls from sync context.
        // Try to get the current Tokio handle, or create a minimal one.
        let rt = tokio::runtime::Handle::try_current()
            .unwrap_or_else(|_| {
                // This branch shouldn't normally be hit since main() uses #[tokio::main],
                // but just in case, we create a current-thread runtime.
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                rt.handle().clone()
            });

        let connection = rt.block_on(async {
            Connection::session()
                .await
                .context("Failed to connect to D-Bus session bus. Is AT-SPI2 running?")
        })?;

        tracing::info!("Connected to D-Bus session bus for AT-SPI2");

        Ok(Self {
            connection,
            registry: Arc::new(DashMap::new()),
            rt,
        })
    }

    // ── Role → ElementType mapping ────────────────────────────────────────

    fn role_to_element_type(role: Role) -> ElementType {
        match role {
            Role::Frame | Role::Window => ElementType::Window,
            Role::PushButton | Role::ToggleButton => ElementType::Button,
            Role::Text | Role::Entry | Role::PasswordText | Role::SpinButton => ElementType::Edit,
            Role::Label | Role::StaticText | Role::Heading | Role::Paragraph => ElementType::Text,
            Role::CheckBox | Role::CheckMenuItem => ElementType::CheckBox,
            Role::RadioButton | Role::RadioMenuItem => ElementType::RadioButton,
            Role::ComboBox => ElementType::ComboBox,
            Role::List => ElementType::ListBox,
            Role::ListItem => ElementType::ListItem,
            Role::Tree | Role::TreeTable => ElementType::TreeView,
            Role::TreeItem => ElementType::TreeItem,
            Role::Menu | Role::MenuBar => ElementType::Menu,
            Role::MenuItem => ElementType::MenuItem,
            Role::PageTabList => ElementType::TabControl,
            Role::PageTab => ElementType::TabItem,
            Role::ToolBar => ElementType::ToolBar,
            Role::StatusBar => ElementType::StatusBar,
            Role::ScrollBar => ElementType::ScrollBar,
            Role::Slider => ElementType::Slider,
            Role::ProgressBar => ElementType::ProgressBar,
            Role::Image | Role::Icon => ElementType::Image,
            Role::Link => ElementType::Link,
            Role::Panel | Role::Filler | Role::Section => ElementType::Group,
            Role::ScrollPane | Role::Viewport => ElementType::Pane,
            Role::Dialog | Role::Alert | Role::FileChooser => ElementType::Dialog,
            Role::DocumentFrame | Role::DocumentWeb => ElementType::Document,
            Role::Table => ElementType::Table,
            _ => ElementType::Unknown,
        }
    }

    // ── Async helpers ─────────────────────────────────────────────────────

    async fn get_accessible_proxy(
        &self,
        bus_name: &str,
        object_path: &str,
    ) -> Result<AccessibleProxy<'_>> {
        AccessibleProxy::builder(&self.connection)
            .destination(bus_name)?
            .path(object_path)?
            .build()
            .await
            .context("Failed to build AccessibleProxy")
    }

    async fn build_element_async(
        &self,
        bus_name: &str,
        object_path: &str,
        with_children: bool,
        depth: u32,
    ) -> Result<UiElement> {
        if depth > 48 {
            let id = Uuid::new_v4().to_string();
            self.registry.insert(
                id.clone(),
                SafeElement {
                    bus_name: bus_name.to_string(),
                    object_path: object_path.to_string(),
                },
            );
            return Ok(UiElement {
                oculos_id: id,
                element_type: ElementType::Unknown,
                label: String::new(),
                value: None,
                text_content: None,
                rect: Rect { x: 0, y: 0, width: 0, height: 0 },
                enabled: false,
                focused: false,
                is_keyboard_focusable: false,
                toggle_state: None,
                is_selected: None,
                expand_state: None,
                range: None,
                automation_id: None,
                class_name: None,
                help_text: None,
                keyboard_shortcut: None,
                actions: vec![],
                children: vec![],
            });
        }

        let proxy = self.get_accessible_proxy(bus_name, object_path).await?;

        let name = proxy.name().await.unwrap_or_default();
        let role = proxy.get_role().await.unwrap_or(Role::Unknown);
        let element_type = Self::role_to_element_type(role);

        // State set
        let states = proxy.get_state().await.unwrap_or_default();
        let enabled = states.contains(atspi::State::Enabled);
        let focused = states.contains(atspi::State::Focused);
        let is_keyboard_focusable = states.contains(atspi::State::Focusable);
        let is_selected_state = states.contains(atspi::State::Selected);
        let is_checked = states.contains(atspi::State::Checked);
        let is_expanded = states.contains(atspi::State::Expanded);
        let is_expandable = states.contains(atspi::State::Expandable);

        // Bounding box via Component interface
        let rect = if let Ok(comp) = ComponentProxy::builder(&self.connection)
            .destination(bus_name)
            .ok()
            .and_then(|b| b.path(object_path).ok())
            .map(|b| self.rt.block_on(b.build()))
        {
            if let Ok(comp) = comp {
                if let Ok(extents) = comp.get_extents(atspi::CoordType::Screen).await {
                    Rect {
                        x: extents.0,
                        y: extents.1,
                        width: extents.2,
                        height: extents.3,
                    }
                } else {
                    Rect { x: 0, y: 0, width: 0, height: 0 }
                }
            } else {
                Rect { x: 0, y: 0, width: 0, height: 0 }
            }
        } else {
            Rect { x: 0, y: 0, width: 0, height: 0 }
        };

        // Value (for text fields, sliders, etc.)
        let value = self.get_text_value(bus_name, object_path).await;

        // Toggle state
        let toggle_state = if element_type == ElementType::CheckBox {
            Some(if is_checked {
                ToggleState::On
            } else {
                ToggleState::Off
            })
        } else {
            None
        };

        // Selection state
        let is_selected = if is_selected_state {
            Some(true)
        } else {
            None
        };

        // Expand state
        let expand_state = if is_expandable {
            Some(if is_expanded {
                ExpandState::Expanded
            } else {
                ExpandState::Collapsed
            })
        } else {
            None
        };

        // Range info (sliders, spinners)
        let range = self.get_range_info(bus_name, object_path).await;

        // Actions
        let actions = self.collect_actions(bus_name, object_path, &element_type, is_keyboard_focusable).await;

        // Description (help text)
        let help_text = proxy.description().await.ok().filter(|s| !s.is_empty());

        // Children
        let children = if with_children {
            let child_count = proxy.child_count().await.unwrap_or(0);
            let mut kids = Vec::with_capacity(child_count as usize);
            for i in 0..child_count {
                if let Ok(child) = proxy.get_child_at_index(i).await {
                    let child_bus = child.0.to_string();
                    let child_path = child.1.to_string();
                    if let Ok(elem) = self
                        .build_element_async(&child_bus, &child_path, true, depth + 1)
                        .await
                    {
                        kids.push(elem);
                    }
                }
            }
            kids
        } else {
            vec![]
        };

        let oculos_id = Uuid::new_v4().to_string();
        self.registry.insert(
            oculos_id.clone(),
            SafeElement {
                bus_name: bus_name.to_string(),
                object_path: object_path.to_string(),
            },
        );

        Ok(UiElement {
            oculos_id,
            element_type,
            label: name,
            value,
            text_content: None,
            rect,
            enabled,
            focused,
            is_keyboard_focusable,
            toggle_state,
            is_selected,
            expand_state,
            range,
            automation_id: None,
            class_name: None,
            help_text,
            keyboard_shortcut: None,
            actions,
            children,
        })
    }

    async fn get_text_value(&self, bus_name: &str, object_path: &str) -> Option<String> {
        let text_proxy = TextProxy::builder(&self.connection)
            .destination(bus_name)
            .ok()?
            .path(object_path)
            .ok()?
            .build()
            .await
            .ok()?;

        let char_count = text_proxy.character_count().await.ok()?;
        if char_count == 0 {
            return None;
        }
        text_proxy
            .get_text(0, char_count)
            .await
            .ok()
            .filter(|s| !s.is_empty())
    }

    async fn get_range_info(&self, bus_name: &str, object_path: &str) -> Option<RangeInfo> {
        let value_proxy = ValueProxy::builder(&self.connection)
            .destination(bus_name)
            .ok()?
            .path(object_path)
            .ok()?
            .build()
            .await
            .ok()?;

        let current = value_proxy.current_value().await.ok()?;
        let minimum = value_proxy.minimum_value().await.unwrap_or(0.0);
        let maximum = value_proxy.maximum_value().await.unwrap_or(100.0);
        let step = value_proxy.minimum_increment().await.unwrap_or(1.0);

        Some(RangeInfo {
            value: current,
            minimum,
            maximum,
            step,
            read_only: false,
        })
    }

    async fn collect_actions(
        &self,
        bus_name: &str,
        object_path: &str,
        element_type: &ElementType,
        focusable: bool,
    ) -> Vec<String> {
        let mut actions = Vec::new();

        if let Ok(action_proxy) = ActionProxy::builder(&self.connection)
            .destination(bus_name)
            .and_then(|b| b.path(object_path))
            .map(|b| async { b.build().await })
        {
            if let Ok(action_proxy) = action_proxy.await {
                let n_actions = action_proxy.n_actions().await.unwrap_or(0);
                for i in 0..n_actions {
                    if let Ok(name) = action_proxy.get_name(i).await {
                        match name.as_str() {
                            "click" | "press" | "activate" => {
                                if !actions.contains(&"click".to_string()) {
                                    actions.push("click".into());
                                }
                            }
                            "toggle" => actions.push("toggle".into()),
                            "expand or contract" | "expand" => actions.push("expand".into()),
                            "collapse" => actions.push("collapse".into()),
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check for editable text
        if let Ok(edit_proxy) = EditableTextProxy::builder(&self.connection)
            .destination(bus_name)
            .and_then(|b| b.path(object_path))
            .map(|b| async { b.build().await })
        {
            if edit_proxy.await.is_ok() {
                actions.push("set-text".into());
                actions.push("send-keys".into());
            }
        }

        // Check for value (slider/spinner)
        if let Ok(value_proxy) = ValueProxy::builder(&self.connection)
            .destination(bus_name)
            .and_then(|b| b.path(object_path))
            .map(|b| async { b.build().await })
        {
            if value_proxy.await.is_ok() {
                if matches!(element_type, ElementType::Slider | ElementType::ProgressBar) {
                    actions.push("set-range".into());
                }
            }
        }

        if focusable {
            actions.push("focus".into());
        }

        actions
    }

    // ── Search helper ─────────────────────────────────────────────────────

    async fn search_elements_async(
        &self,
        bus_name: &str,
        object_path: &str,
        query: Option<&str>,
        element_type: Option<&ElementType>,
        interactive_only: bool,
        results: &mut Vec<UiElement>,
        depth: u32,
    ) {
        if depth > 48 || results.len() >= 500 {
            return;
        }

        if let Ok(elem) = self
            .build_element_async(bus_name, object_path, false, depth)
            .await
        {
            let query_lower = query.map(|q| q.to_lowercase());
            let mut matches = true;

            if let Some(ref q) = query_lower {
                let label_match = elem.label.to_lowercase().contains(q.as_str());
                let aid_match = elem
                    .automation_id
                    .as_ref()
                    .map(|a| a.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                if !label_match && !aid_match {
                    matches = false;
                }
            }

            if let Some(wanted) = element_type {
                if &elem.element_type != wanted {
                    matches = false;
                }
            }

            if interactive_only && elem.actions.is_empty() {
                matches = false;
            }

            if matches {
                results.push(elem);
            }
        }

        // Recurse into children
        if let Ok(proxy) = self.get_accessible_proxy(bus_name, object_path).await {
            let child_count = proxy.child_count().await.unwrap_or(0);
            for i in 0..child_count {
                if let Ok(child) = proxy.get_child_at_index(i).await {
                    let child_bus = child.0.to_string();
                    let child_path = child.1.to_string();
                    Box::pin(self.search_elements_async(
                        &child_bus,
                        &child_path,
                        query,
                        element_type,
                        interactive_only,
                        results,
                        depth + 1,
                    ))
                    .await;
                }
            }
        }
    }

    // ── Find app root for a PID ───────────────────────────────────────────

    async fn find_app_root(&self, pid: u32) -> Result<(String, String)> {
        // Connect to AT-SPI2 registry to find the application with given PID
        let registry = AccessibleProxy::builder(&self.connection)
            .destination("org.a11y.atspi.Registry")?
            .path("/org/a11y/atspi/accessible/root")?
            .build()
            .await
            .context("Failed to connect to AT-SPI2 registry")?;

        let child_count = registry.child_count().await.unwrap_or(0);
        for i in 0..child_count {
            if let Ok(child) = registry.get_child_at_index(i).await {
                let child_bus = child.0.to_string();
                let child_path = child.1.to_string();

                if let Ok(app_proxy) = self
                    .get_accessible_proxy(&child_bus, &child_path)
                    .await
                {
                    // AT-SPI2 applications expose their PID via the Application interface
                    if let Ok(app_pid) = ApplicationProxy::builder(&self.connection)
                        .destination(&child_bus)
                        .and_then(|b| b.path(&child_path))
                        .map(|b| async { b.build().await })
                    {
                        if let Ok(app_proxy) = app_pid.await {
                            if let Ok(p) = app_proxy.id().await {
                                if p as u32 == pid {
                                    return Ok((child_bus, child_path));
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow!("No AT-SPI2 application found for PID {}", pid))
    }

    // ── Sync wrappers ─────────────────────────────────────────────────────

    fn block_on<F: std::future::Future<Output = T>, T>(&self, f: F) -> T {
        self.rt.block_on(f)
    }
}

// ── Application proxy (for PID lookup) ────────────────────────────────────────

use atspi::proxy::application::ApplicationProxy;

// ── UiBackend implementation ──────────────────────────────────────────────────

impl UiBackend for LinuxUiBackend {
    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        self.block_on(async {
            let registry = AccessibleProxy::builder(&self.connection)
                .destination("org.a11y.atspi.Registry")?
                .path("/org/a11y/atspi/accessible/root")?
                .build()
                .await
                .context("Failed to connect to AT-SPI2 registry")?;

            let child_count = registry.child_count().await.unwrap_or(0);
            let mut windows = Vec::new();

            for i in 0..child_count {
                if let Ok(child) = registry.get_child_at_index(i).await {
                    let child_bus = child.0.to_string();
                    let child_path = child.1.to_string();

                    if let Ok(app_proxy) = self
                        .get_accessible_proxy(&child_bus, &child_path)
                        .await
                    {
                        let app_name = app_proxy.name().await.unwrap_or_default();
                        if app_name.is_empty() {
                            continue;
                        }

                        // Get PID
                        let pid = if let Ok(app_iface) =
                            ApplicationProxy::builder(&self.connection)
                                .destination(&child_bus)
                                .and_then(|b| b.path(&child_path))
                                .map(|b| async { b.build().await })
                        {
                            if let Ok(ap) = app_iface.await {
                                ap.id().await.unwrap_or(0) as u32
                            } else {
                                0
                            }
                        } else {
                            0
                        };

                        // Enumerate this app's windows (children with Frame/Window role)
                        let app_child_count =
                            app_proxy.child_count().await.unwrap_or(0);
                        let mut found_window = false;

                        for j in 0..app_child_count {
                            if let Ok(win) = app_proxy.get_child_at_index(j).await {
                                let win_bus = win.0.to_string();
                                let win_path = win.1.to_string();

                                if let Ok(win_proxy) = self
                                    .get_accessible_proxy(&win_bus, &win_path)
                                    .await
                                {
                                    let role =
                                        win_proxy.get_role().await.unwrap_or(Role::Unknown);
                                    if matches!(role, Role::Frame | Role::Window | Role::Dialog)
                                    {
                                        let title =
                                            win_proxy.name().await.unwrap_or_default();

                                        // Get extents via Component
                                        let rect = self
                                            .get_component_rect(&win_bus, &win_path)
                                            .await;

                                        windows.push(WindowInfo {
                                            pid,
                                            hwnd: 0,
                                            title,
                                            exe_name: app_name.clone(),
                                            rect,
                                            visible: true,
                                        });
                                        found_window = true;
                                    }
                                }
                            }
                        }

                        if !found_window && pid > 0 {
                            windows.push(WindowInfo {
                                pid,
                                hwnd: 0,
                                title: app_name.clone(),
                                exe_name: app_name,
                                rect: Rect { x: 0, y: 0, width: 0, height: 0 },
                                visible: true,
                            });
                        }
                    }
                }
            }

            Ok(windows)
        })
    }

    fn get_ui_tree(&self, pid: u32) -> Result<UiElement> {
        self.block_on(async {
            let (bus, path) = self.find_app_root(pid).await?;
            self.build_element_async(&bus, &path, true, 0).await
        })
    }

    fn get_ui_tree_hwnd(&self, _hwnd: usize) -> Result<UiElement> {
        Err(anyhow!(
            "Linux does not use window handles (HWND). Use the PID-based endpoint instead."
        ))
    }

    fn find_elements(
        &self,
        pid: u32,
        query: Option<&str>,
        element_type: Option<&ElementType>,
        interactive_only: bool,
    ) -> Result<Vec<UiElement>> {
        self.block_on(async {
            let (bus, path) = self.find_app_root(pid).await?;
            let mut results = Vec::new();
            self.search_elements_async(
                &bus,
                &path,
                query,
                element_type,
                interactive_only,
                &mut results,
                0,
            )
            .await;
            Ok(results)
        })
    }

    fn find_elements_hwnd(
        &self,
        _hwnd: usize,
        _query: Option<&str>,
        _element_type: Option<&ElementType>,
        _interactive_only: bool,
    ) -> Result<Vec<UiElement>> {
        Err(anyhow!(
            "Linux does not use window handles (HWND). Use the PID-based endpoint instead."
        ))
    }

    fn click_element(&self, oculos_id: &str) -> Result<()> {
        let entry = self.registry.get(oculos_id).ok_or_else(|| {
            anyhow!("Element '{}' not found in registry", oculos_id)
        })?;
        let bus = entry.value().bus_name.clone();
        let path = entry.value().object_path.clone();
        drop(entry);

        self.block_on(async {
            let action_proxy = ActionProxy::builder(&self.connection)
                .destination(&bus)?
                .path(&path)?
                .build()
                .await
                .context("Element does not support Action interface")?;

            let n_actions = action_proxy.n_actions().await.unwrap_or(0);
            for i in 0..n_actions {
                if let Ok(name) = action_proxy.get_name(i).await {
                    if matches!(name.as_str(), "click" | "press" | "activate") {
                        action_proxy.do_action(i).await?;
                        return Ok(());
                    }
                }
            }

            // Fallback: try the first action
            if n_actions > 0 {
                action_proxy.do_action(0).await?;
                return Ok(());
            }

            Err(anyhow!("No clickable action found on element '{}'", oculos_id))
        })
    }

    fn set_text(&self, oculos_id: &str, text: &str) -> Result<()> {
        let entry = self.registry.get(oculos_id).ok_or_else(|| {
            anyhow!("Element '{}' not found in registry", oculos_id)
        })?;
        let bus = entry.value().bus_name.clone();
        let path = entry.value().object_path.clone();
        drop(entry);

        self.block_on(async {
            let edit_proxy = EditableTextProxy::builder(&self.connection)
                .destination(&bus)?
                .path(&path)?
                .build()
                .await
                .context("Element does not support EditableText interface")?;

            // Get current text length to replace all
            let text_proxy = TextProxy::builder(&self.connection)
                .destination(&bus)?
                .path(&path)?
                .build()
                .await
                .context("Element does not support Text interface")?;

            let char_count = text_proxy.character_count().await.unwrap_or(0);
            if char_count > 0 {
                let _ = edit_proxy.delete_text(0, char_count).await;
            }
            edit_proxy.insert_text(0, text, text.len() as i32).await?;

            Ok(())
        })
    }

    fn send_keys(&self, oculos_id: &str, text: &str) -> Result<()> {
        self.focus_element(oculos_id)?;
        std::thread::sleep(std::time::Duration::from_millis(60));
        send_key_sequence_linux(text);
        Ok(())
    }

    fn focus_element(&self, oculos_id: &str) -> Result<()> {
        let entry = self.registry.get(oculos_id).ok_or_else(|| {
            anyhow!("Element '{}' not found in registry", oculos_id)
        })?;
        let bus = entry.value().bus_name.clone();
        let path = entry.value().object_path.clone();
        drop(entry);

        self.block_on(async {
            let comp_proxy = ComponentProxy::builder(&self.connection)
                .destination(&bus)?
                .path(&path)?
                .build()
                .await
                .context("Element does not support Component interface")?;

            comp_proxy.grab_focus().await?;
            Ok(())
        })
    }

    fn toggle_element(&self, oculos_id: &str) -> Result<()> {
        self.click_element(oculos_id)
    }

    fn expand_element(&self, oculos_id: &str) -> Result<()> {
        let entry = self.registry.get(oculos_id).ok_or_else(|| {
            anyhow!("Element '{}' not found in registry", oculos_id)
        })?;
        let bus = entry.value().bus_name.clone();
        let path = entry.value().object_path.clone();
        drop(entry);

        self.block_on(async {
            let action_proxy = ActionProxy::builder(&self.connection)
                .destination(&bus)?
                .path(&path)?
                .build()
                .await
                .context("Element does not support Action interface")?;

            let n_actions = action_proxy.n_actions().await.unwrap_or(0);
            for i in 0..n_actions {
                if let Ok(name) = action_proxy.get_name(i).await {
                    if matches!(name.as_str(), "expand or contract" | "expand" | "open") {
                        action_proxy.do_action(i).await?;
                        return Ok(());
                    }
                }
            }

            Err(anyhow!("No expand action found on element '{}'", oculos_id))
        })
    }

    fn collapse_element(&self, oculos_id: &str) -> Result<()> {
        // AT-SPI2 often uses "expand or contract" as a toggle
        self.expand_element(oculos_id)
    }

    fn select_element(&self, oculos_id: &str) -> Result<()> {
        self.click_element(oculos_id)
    }

    fn set_range(&self, oculos_id: &str, value: f64) -> Result<()> {
        let entry = self.registry.get(oculos_id).ok_or_else(|| {
            anyhow!("Element '{}' not found in registry", oculos_id)
        })?;
        let bus = entry.value().bus_name.clone();
        let path = entry.value().object_path.clone();
        drop(entry);

        self.block_on(async {
            let value_proxy = ValueProxy::builder(&self.connection)
                .destination(&bus)?
                .path(&path)?
                .build()
                .await
                .context("Element does not support Value interface")?;

            value_proxy.set_current_value(value).await?;
            Ok(())
        })
    }

    fn scroll_element(&self, oculos_id: &str, direction: &str) -> Result<()> {
        // AT-SPI2 doesn't have a native scroll API. Use XDotool or synthetic key events.
        let key = match direction {
            "up" => "Up",
            "down" => "Down",
            "left" => "Left",
            "right" => "Right",
            "page-up" => "Page_Up",
            "page-down" => "Page_Down",
            other => {
                return Err(anyhow!(
                    "Unknown scroll direction '{}'. Use: up, down, left, right, page-up, page-down",
                    other
                ))
            }
        };

        self.focus_element(oculos_id)?;
        std::thread::sleep(std::time::Duration::from_millis(30));
        send_key_sequence_linux(&format!("{{{}}}", key));
        Ok(())
    }

    fn scroll_into_view(&self, _oculos_id: &str) -> Result<()> {
        Err(anyhow!(
            "scroll-into-view is not natively supported on Linux AT-SPI2. \
             Try scrolling the parent container manually."
        ))
    }

    fn focus_window(&self, pid: u32) -> Result<()> {
        // Use wmctrl or xdotool to activate window
        let output = std::process::Command::new("xdotool")
            .args(["search", "--pid", &pid.to_string(), "--onlyvisible", "windowactivate"])
            .output();

        match output {
            Ok(o) if o.status.success() => Ok(()),
            _ => {
                // Fallback: try wmctrl
                let _ = std::process::Command::new("wmctrl")
                    .args(["-i", "-a", &format!("0x{:08x}", pid)])
                    .output();
                Ok(())
            }
        }
    }

    fn close_window(&self, pid: u32) -> Result<()> {
        // Use xdotool to close window
        let output = std::process::Command::new("xdotool")
            .args([
                "search",
                "--pid",
                &pid.to_string(),
                "--onlyvisible",
                "windowclose",
            ])
            .output();

        match output {
            Ok(o) if o.status.success() => Ok(()),
            _ => Err(anyhow!(
                "Failed to close window for PID {}. Is xdotool installed?",
                pid
            )),
        }
    }
}

// ── Component rect helper ─────────────────────────────────────────────────────

impl LinuxUiBackend {
    async fn get_component_rect(&self, bus_name: &str, object_path: &str) -> Rect {
        if let Ok(comp) = ComponentProxy::builder(&self.connection)
            .destination(bus_name)
            .and_then(|b| b.path(object_path))
        {
            if let Ok(comp) = comp.build().await {
                if let Ok(extents) = comp.get_extents(atspi::CoordType::Screen).await {
                    return Rect {
                        x: extents.0,
                        y: extents.1,
                        width: extents.2,
                        height: extents.3,
                    };
                }
            }
        }
        Rect { x: 0, y: 0, width: 0, height: 0 }
    }
}

// ── Linux keyboard simulation via xdotool ─────────────────────────────────────

fn send_key_sequence_linux(text: &str) {
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut key_name = String::new();
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == '}' {
                    break;
                }
                key_name.push(c);
            }
            send_special_key_linux(&key_name);
        } else {
            // Regular character
            let _ = std::process::Command::new("xdotool")
                .args(["type", "--clearmodifiers", &ch.to_string()])
                .output();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

fn send_special_key_linux(key_name: &str) {
    let xdotool_key = match key_name {
        "ENTER" | "RETURN" => "Return",
        "TAB" => "Tab",
        "ESC" | "ESCAPE" => "Escape",
        "SPACE" => "space",
        "DELETE" => "Delete",
        "BACKSPACE" => "BackSpace",
        "UP" => "Up",
        "DOWN" => "Down",
        "LEFT" => "Left",
        "RIGHT" => "Right",
        "HOME" => "Home",
        "END" => "End",
        "PGUP" => "Page_Up",
        "PGDN" => "Page_Down",
        "F1" => "F1",
        "F2" => "F2",
        "F3" => "F3",
        "F4" => "F4",
        "F5" => "F5",
        "F6" => "F6",
        "F7" => "F7",
        "F8" => "F8",
        "F9" => "F9",
        "F10" => "F10",
        "F11" => "F11",
        "F12" => "F12",
        // Modifier combos
        s if s.contains('+') => {
            let parts: Vec<&str> = s.splitn(2, '+').collect();
            let modifier = match parts[0] {
                "CTRL" => "ctrl",
                "ALT" => "alt",
                "SHIFT" => "shift",
                "WIN" | "SUPER" => "super",
                other => other,
            };
            let key = parts.get(1).unwrap_or(&"").to_lowercase();
            let combo = format!("{}+{}", modifier, key);
            let _ = std::process::Command::new("xdotool")
                .args(["key", "--clearmodifiers", &combo])
                .output();
            return;
        }
        _ => return,
    };

    let _ = std::process::Command::new("xdotool")
        .args(["key", "--clearmodifiers", xdotool_key])
        .output();
    std::thread::sleep(std::time::Duration::from_millis(20));
}
