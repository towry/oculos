use anyhow::{anyhow, Context, Result};
use dashmap::DashMap;
use std::{mem::size_of, sync::Arc};
use uuid::Uuid;

use windows::{
    core::BSTR,
    Win32::{
        Foundation::{BOOL, HWND, LPARAM, RECT},
        Graphics::Gdi::{
            BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreatePen, DeleteDC, DeleteObject,
            GetDC, GetDIBits, ReleaseDC, SelectObject, SetROP2, BITMAPINFO, BITMAPINFOHEADER,
            BI_RGB, DIB_RGB_COLORS, PS_SOLID, R2_NOTXORPEN, SRCCOPY,
        },
        System::{
            Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED},
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::{
            Accessibility::{
                CUIAutomation, ExpandCollapseState_Collapsed, ExpandCollapseState_Expanded,
                ExpandCollapseState_LeafNode, ExpandCollapseState_PartiallyExpanded, IUIAutomation,
                IUIAutomationCondition, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
                IUIAutomationInvokePattern, IUIAutomationRangeValuePattern,
                IUIAutomationScrollItemPattern, IUIAutomationScrollPattern,
                IUIAutomationSelectionItemPattern, IUIAutomationTextPattern,
                IUIAutomationTogglePattern, IUIAutomationValuePattern, IUIAutomationWindowPattern,
                ScrollAmount_LargeDecrement, ScrollAmount_LargeIncrement, ScrollAmount_NoAmount,
                ScrollAmount_SmallDecrement, ScrollAmount_SmallIncrement,
                ToggleState_Indeterminate, ToggleState_Off, ToggleState_On, TreeScope_Children,
                TreeScope_Subtree, UIA_ExpandCollapsePatternId, UIA_InvokePatternId,
                UIA_RangeValuePatternId, UIA_ScrollItemPatternId, UIA_ScrollPatternId,
                UIA_SelectionItemPatternId, UIA_TextPatternId, UIA_TogglePatternId,
                UIA_ValuePatternId, UIA_WindowPatternId, UIA_CONTROLTYPE_ID,
            },
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY,
            },
            WindowsAndMessaging::{
                EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
                IsWindowVisible, SetForegroundWindow,
            },
        },
    },
};

use crate::{
    platform::UiBackend,
    types::{ElementType, ExpandState, RangeInfo, Rect, ToggleState, UiElement, WindowInfo},
};

// ── Element registry ──────────────────────────────────────────────────────────

struct SafeElement(IUIAutomationElement);
unsafe impl Send for SafeElement {}
unsafe impl Sync for SafeElement {}

type IdRegistry = Arc<DashMap<String, SafeElement>>;

// ── Backend ───────────────────────────────────────────────────────────────────

pub struct WindowsUiBackend {
    automation: IUIAutomation,
    registry: IdRegistry,
}

unsafe impl Send for WindowsUiBackend {}
unsafe impl Sync for WindowsUiBackend {}

impl WindowsUiBackend {
    pub fn new() -> Result<Self> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            Ok(Self {
                automation,
                registry: Arc::new(DashMap::new()),
            })
        }
    }

    // ── Control type mapping ──────────────────────────────────────────────

    fn control_type(id: UIA_CONTROLTYPE_ID) -> ElementType {
        match id.0 {
            50000 => ElementType::Button,
            50002 => ElementType::CheckBox,
            50003 => ElementType::ComboBox,
            50004 => ElementType::Edit,
            50005 => ElementType::Link,
            50006 => ElementType::Image,
            50007 => ElementType::ListItem,
            50008 => ElementType::ListBox,
            50009 => ElementType::Menu,
            50010 => ElementType::Menu,
            50011 => ElementType::MenuItem,
            50012 => ElementType::ProgressBar,
            50013 => ElementType::RadioButton,
            50014 => ElementType::ScrollBar,
            50015 => ElementType::Slider,
            50016 => ElementType::Slider,
            50017 => ElementType::StatusBar,
            50018 => ElementType::TabControl,
            50019 => ElementType::TabItem,
            50020 => ElementType::Text,
            50021 => ElementType::ToolBar,
            50023 => ElementType::TreeItem,
            50024 => ElementType::TreeItem,
            50025 => ElementType::Custom,
            50026 => ElementType::Group,
            50028 => ElementType::DataGrid,
            50029 => ElementType::DataItem,
            50030 => ElementType::Document,
            50032 => ElementType::Window,
            50033 => ElementType::Pane,
            50035 => ElementType::HeaderItem,
            50036 => ElementType::Table,
            _ => ElementType::Unknown,
        }
    }

    // ── Pattern helpers ───────────────────────────────────────────────────

    /// Collect all available patterns and build the `actions` list + state.
    unsafe fn collect_patterns(element: &IUIAutomationElement) -> PatternInfo {
        let mut info = PatternInfo::default();

        // InvokePattern → "click"
        if let Ok(raw) = element.GetCurrentPattern(UIA_InvokePatternId) {
            if windows::core::Interface::cast::<IUIAutomationInvokePattern>(&raw).is_ok() {
                info.actions.push("click".into());
            }
        }

        // ValuePattern → "set-text"
        if let Ok(raw) = element.GetCurrentPattern(UIA_ValuePatternId) {
            if let Ok(vp) = windows::core::Interface::cast::<IUIAutomationValuePattern>(&raw) {
                let read_only = vp.CurrentIsReadOnly().unwrap_or(BOOL(1)).as_bool();
                if !read_only {
                    info.actions.push("set-text".into());
                }
                info.value = vp
                    .CurrentValue()
                    .map(|b| b.to_string())
                    .ok()
                    .filter(|s| !s.is_empty());
            }
        }

        // TogglePattern → "toggle" + toggle_state
        if let Ok(raw) = element.GetCurrentPattern(UIA_TogglePatternId) {
            if let Ok(tp) = windows::core::Interface::cast::<IUIAutomationTogglePattern>(&raw) {
                info.actions.push("toggle".into());
                info.toggle_state = tp.CurrentToggleState().ok().map(|s| match s {
                    x if x == ToggleState_On => ToggleState::On,
                    x if x == ToggleState_Off => ToggleState::Off,
                    _ => ToggleState::Indeterminate,
                });
            }
        }

        // ExpandCollapsePattern → "expand"/"collapse" + expand_state
        if let Ok(raw) = element.GetCurrentPattern(UIA_ExpandCollapsePatternId) {
            if let Ok(ep) =
                windows::core::Interface::cast::<IUIAutomationExpandCollapsePattern>(&raw)
            {
                let state = ep.CurrentExpandCollapseState().ok();
                info.expand_state = state.map(|s| match s {
                    x if x == ExpandCollapseState_Collapsed => ExpandState::Collapsed,
                    x if x == ExpandCollapseState_Expanded => ExpandState::Expanded,
                    x if x == ExpandCollapseState_PartiallyExpanded => {
                        ExpandState::PartiallyExpanded
                    }
                    _ => ExpandState::LeafNode,
                });
                match &info.expand_state {
                    Some(ExpandState::Collapsed) | Some(ExpandState::PartiallyExpanded) => {
                        info.actions.push("expand".into());
                    }
                    Some(ExpandState::Expanded) => {
                        info.actions.push("collapse".into());
                    }
                    _ => {}
                }
            }
        }

        // SelectionItemPattern → "select" + is_selected
        if let Ok(raw) = element.GetCurrentPattern(UIA_SelectionItemPatternId) {
            if let Ok(sp) =
                windows::core::Interface::cast::<IUIAutomationSelectionItemPattern>(&raw)
            {
                info.actions.push("select".into());
                info.is_selected = sp.CurrentIsSelected().ok().map(|b| b.as_bool());
            }
        }

        // RangeValuePattern → "set-range" + range info
        if let Ok(raw) = element.GetCurrentPattern(UIA_RangeValuePatternId) {
            if let Ok(rp) = windows::core::Interface::cast::<IUIAutomationRangeValuePattern>(&raw) {
                let read_only = rp.CurrentIsReadOnly().unwrap_or(BOOL(1)).as_bool();
                if !read_only {
                    info.actions.push("set-range".into());
                }
                info.range = Some(RangeInfo {
                    value: rp.CurrentValue().unwrap_or(0.0),
                    minimum: rp.CurrentMinimum().unwrap_or(0.0),
                    maximum: rp.CurrentMaximum().unwrap_or(100.0),
                    step: rp.CurrentSmallChange().unwrap_or(1.0),
                    read_only,
                });
            }
        }

        // ScrollPattern → the element is a scrollable container
        if let Ok(raw) = element.GetCurrentPattern(UIA_ScrollPatternId) {
            if windows::core::Interface::cast::<IUIAutomationScrollPattern>(&raw).is_ok() {
                info.actions.push("scroll".into());
            }
        }

        // ScrollItemPattern → element can be scrolled into view
        if let Ok(raw) = element.GetCurrentPattern(UIA_ScrollItemPatternId) {
            if windows::core::Interface::cast::<IUIAutomationScrollItemPattern>(&raw).is_ok() {
                info.actions.push("scroll-into-view".into());
            }
        }

        // TextPattern → read rich text content
        if let Ok(raw) = element.GetCurrentPattern(UIA_TextPatternId) {
            if let Ok(tp) = windows::core::Interface::cast::<IUIAutomationTextPattern>(&raw) {
                if let Ok(range) = tp.DocumentRange() {
                    info.text_content = range
                        .GetText(-1)
                        .map(|b| b.to_string())
                        .ok()
                        .filter(|s| !s.is_empty());
                }
            }
        }

        // Keyboard focusable → "focus"
        info.is_keyboard_focusable = element
            .CurrentIsKeyboardFocusable()
            .unwrap_or(BOOL(0))
            .as_bool();
        if info.is_keyboard_focusable {
            info.actions.push("focus".into());
        }

        // Any focusable Edit or Custom can receive send-keys
        let ctrl_id = element
            .CurrentControlType()
            .unwrap_or(UIA_CONTROLTYPE_ID(0));
        if info.is_keyboard_focusable
            && matches!(
                Self::control_type(ctrl_id),
                ElementType::Edit | ElementType::Document | ElementType::Custom
            )
        {
            info.actions.push("send-keys".into());
        }

        info
    }

    // ── Tree builder ──────────────────────────────────────────────────────

    fn build_tree(&self, element: IUIAutomationElement, depth: u32) -> Result<UiElement> {
        if depth > 48 {
            let id = Uuid::new_v4().to_string();
            self.registry.insert(id.clone(), SafeElement(element));
            return Ok(UiElement {
                oculos_id: id,
                element_type: ElementType::Unknown,
                label: String::new(),
                value: None,
                text_content: None,
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                },
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

        unsafe {
            let label = element
                .CurrentName()
                .map(|b| b.to_string())
                .unwrap_or_default();

            let ctrl_id = element
                .CurrentControlType()
                .unwrap_or(UIA_CONTROLTYPE_ID(0));
            let element_type = Self::control_type(ctrl_id);

            let enabled = element.CurrentIsEnabled().unwrap_or(BOOL(1)).as_bool();
            let focused = element
                .CurrentHasKeyboardFocus()
                .unwrap_or(BOOL(0))
                .as_bool();

            let automation_id = element
                .CurrentAutomationId()
                .map(|b| b.to_string())
                .ok()
                .filter(|s| !s.is_empty());

            let class_name = element
                .CurrentClassName()
                .map(|b| b.to_string())
                .ok()
                .filter(|s| !s.is_empty());

            let help_text = element
                .CurrentHelpText()
                .map(|b| b.to_string())
                .ok()
                .filter(|s| !s.is_empty());

            // CurrentKeyboardShortcut is not exposed in windows-rs 0.58
            let keyboard_shortcut: Option<String> = None;

            let bounding = element.CurrentBoundingRectangle().unwrap_or(RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            });
            let rect = Rect {
                x: bounding.left,
                y: bounding.top,
                width: bounding.right - bounding.left,
                height: bounding.bottom - bounding.top,
            };

            // ── Collect pattern info ──────────────────────────────────────
            let pinfo = Self::collect_patterns(&element);

            // ── Children ──────────────────────────────────────────────────
            let true_cond: IUIAutomationCondition = self.automation.CreateTrueCondition()?;
            let arr = element.FindAll(TreeScope_Children, &true_cond)?;
            let count = arr.Length().unwrap_or(0);

            let mut children = Vec::with_capacity(count as usize);
            for i in 0..count {
                if let Ok(child) = arr.GetElement(i) {
                    if let Ok(node) = self.build_tree(child, depth + 1) {
                        children.push(node);
                    }
                }
            }

            // ── Register ──────────────────────────────────────────────────
            let oculos_id = Uuid::new_v4().to_string();
            self.registry
                .insert(oculos_id.clone(), SafeElement(element));

            Ok(UiElement {
                oculos_id,
                element_type,
                label,
                value: pinfo.value,
                text_content: pinfo.text_content,
                rect,
                enabled,
                focused,
                is_keyboard_focusable: pinfo.is_keyboard_focusable,
                toggle_state: pinfo.toggle_state,
                is_selected: pinfo.is_selected,
                expand_state: pinfo.expand_state,
                range: pinfo.range,
                automation_id,
                class_name,
                help_text,
                keyboard_shortcut,
                actions: pinfo.actions,
                children,
            })
        }
    }

    /// Build a shallow (no children) UiElement from a raw element.
    /// Used for flat find results.
    fn build_flat(&self, element: IUIAutomationElement) -> Result<UiElement> {
        unsafe {
            let label = element
                .CurrentName()
                .map(|b| b.to_string())
                .unwrap_or_default();
            let ctrl_id = element
                .CurrentControlType()
                .unwrap_or(UIA_CONTROLTYPE_ID(0));
            let element_type = Self::control_type(ctrl_id);
            let enabled = element.CurrentIsEnabled().unwrap_or(BOOL(1)).as_bool();
            let focused = element
                .CurrentHasKeyboardFocus()
                .unwrap_or(BOOL(0))
                .as_bool();
            let automation_id = element
                .CurrentAutomationId()
                .map(|b| b.to_string())
                .ok()
                .filter(|s| !s.is_empty());
            let class_name = element
                .CurrentClassName()
                .map(|b| b.to_string())
                .ok()
                .filter(|s| !s.is_empty());
            let help_text = element
                .CurrentHelpText()
                .map(|b| b.to_string())
                .ok()
                .filter(|s| !s.is_empty());
            // CurrentKeyboardShortcut is not exposed in windows-rs 0.58
            let keyboard_shortcut: Option<String> = None;
            let bounding = element.CurrentBoundingRectangle().unwrap_or(RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            });
            let rect = Rect {
                x: bounding.left,
                y: bounding.top,
                width: bounding.right - bounding.left,
                height: bounding.bottom - bounding.top,
            };
            let pinfo = Self::collect_patterns(&element);
            let oculos_id = Uuid::new_v4().to_string();
            self.registry
                .insert(oculos_id.clone(), SafeElement(element));

            Ok(UiElement {
                oculos_id,
                element_type,
                label,
                value: pinfo.value,
                text_content: pinfo.text_content,
                rect,
                enabled,
                focused,
                is_keyboard_focusable: pinfo.is_keyboard_focusable,
                toggle_state: pinfo.toggle_state,
                is_selected: pinfo.is_selected,
                expand_state: pinfo.expand_state,
                range: pinfo.range,
                automation_id,
                class_name,
                help_text,
                keyboard_shortcut,
                actions: pinfo.actions,
                children: vec![],
            })
        }
    }

    // ── Registry lookup ───────────────────────────────────────────────────

    fn find_element(&self, oculos_id: &str) -> Result<IUIAutomationElement> {
        let entry = self.registry.get(oculos_id).ok_or_else(|| {
            anyhow!(
                "Element '{}' not found — refresh via GET /windows/{{pid}}/tree first.",
                oculos_id
            )
        })?;
        Ok(entry.value().0.clone())
    }

    // ── Shared search core ────────────────────────────────────────────────

    fn find_elements_on_root(
        &self,
        hwnd: HWND,
        query: Option<&str>,
        element_type: Option<&ElementType>,
        interactive_only: bool,
    ) -> Result<Vec<UiElement>> {
        let all_elements = unsafe {
            let root = self
                .automation
                .ElementFromHandle(hwnd)
                .context("ElementFromHandle failed")?;
            let true_cond: IUIAutomationCondition = self.automation.CreateTrueCondition()?;
            root.FindAll(TreeScope_Subtree, &true_cond)?
        };

        let count = unsafe { all_elements.Length().unwrap_or(0) };
        let query_lower = query.map(|q| q.to_lowercase());

        let mut results = Vec::new();
        for i in 0..count {
            let Ok(raw_elem) = (unsafe { all_elements.GetElement(i) }) else {
                continue;
            };

            if let Some(wanted_type) = element_type {
                let ctrl_id = unsafe {
                    raw_elem
                        .CurrentControlType()
                        .unwrap_or(UIA_CONTROLTYPE_ID(0))
                };
                if &Self::control_type(ctrl_id) != wanted_type {
                    continue;
                }
            }

            if let Some(ref q) = query_lower {
                let label = unsafe {
                    raw_elem
                        .CurrentName()
                        .map(|b| b.to_string())
                        .unwrap_or_default()
                        .to_lowercase()
                };
                let aid = unsafe {
                    raw_elem
                        .CurrentAutomationId()
                        .map(|b| b.to_string())
                        .unwrap_or_default()
                        .to_lowercase()
                };
                if !label.contains(q.as_str()) && !aid.contains(q.as_str()) {
                    continue;
                }
            }

            let Ok(ui_elem) = self.build_flat(raw_elem) else {
                continue;
            };

            if interactive_only && ui_elem.actions.is_empty() {
                continue;
            }

            results.push(ui_elem);
        }

        Ok(results)
    }
}

// ── Pattern info accumulator ──────────────────────────────────────────────────

#[derive(Default)]
struct PatternInfo {
    actions: Vec<String>,
    value: Option<String>,
    text_content: Option<String>,
    toggle_state: Option<ToggleState>,
    is_selected: Option<bool>,
    expand_state: Option<ExpandState>,
    range: Option<RangeInfo>,
    is_keyboard_focusable: bool,
}

// ── UiBackend implementation ──────────────────────────────────────────────────

impl UiBackend for WindowsUiBackend {
    // ── Discovery ─────────────────────────────────────────────────────────

    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        let hwnd_list: Vec<HWND> = unsafe {
            let mut list: Vec<HWND> = Vec::new();
            let ptr = &mut list as *mut Vec<HWND> as isize;
            let _ = EnumWindows(Some(enum_windows_cb), LPARAM(ptr));
            list
        };

        let mut result = Vec::new();
        for hwnd in hwnd_list {
            unsafe {
                if !IsWindowVisible(hwnd).as_bool() {
                    continue;
                }
                let mut title_buf = [0u16; 512];
                let len = GetWindowTextW(hwnd, &mut title_buf);
                if len == 0 {
                    continue;
                }
                let title = String::from_utf16_lossy(&title_buf[..len as usize]);
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                let mut wr = RECT::default();
                let _ = GetWindowRect(hwnd, &mut wr);
                result.push(WindowInfo {
                    pid,
                    hwnd: hwnd.0 as usize,
                    title,
                    exe_name: get_exe_name(pid),
                    rect: Rect {
                        x: wr.left,
                        y: wr.top,
                        width: wr.right - wr.left,
                        height: wr.bottom - wr.top,
                    },
                    visible: true,
                });
            }
        }
        Ok(result)
    }

    fn get_ui_tree(&self, pid: u32) -> Result<UiElement> {
        let hwnd = find_main_window(pid)
            .ok_or_else(|| anyhow!("No visible window found for PID {}", pid))?;
        unsafe {
            let element = self
                .automation
                .ElementFromHandle(hwnd)
                .context("ElementFromHandle failed")?;
            self.build_tree(element, 0)
        }
    }

    fn get_ui_tree_hwnd(&self, hwnd: usize) -> Result<UiElement> {
        unsafe {
            let element = self
                .automation
                .ElementFromHandle(HWND(hwnd as *mut core::ffi::c_void))
                .context("ElementFromHandle failed")?;
            self.build_tree(element, 0)
        }
    }

    fn find_elements(
        &self,
        pid: u32,
        query: Option<&str>,
        element_type: Option<&ElementType>,
        interactive_only: bool,
    ) -> Result<Vec<UiElement>> {
        let hwnd = find_main_window(pid)
            .ok_or_else(|| anyhow!("No visible window found for PID {}", pid))?;
        self.find_elements_on_root(hwnd, query, element_type, interactive_only)
    }

    fn find_elements_hwnd(
        &self,
        hwnd: usize,
        query: Option<&str>,
        element_type: Option<&ElementType>,
        interactive_only: bool,
    ) -> Result<Vec<UiElement>> {
        self.find_elements_on_root(
            HWND(hwnd as *mut core::ffi::c_void),
            query,
            element_type,
            interactive_only,
        )
    }

    // ── Basic interactions ─────────────────────────────────────────────────

    fn click_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            if let Ok(raw) = elem.GetCurrentPattern(UIA_InvokePatternId) {
                let inv: IUIAutomationInvokePattern = windows::core::Interface::cast(&raw)?;
                inv.Invoke()?;
                return Ok(());
            }
            // Fallback: give focus (triggers default action for some elements)
            elem.SetFocus()?;
        }
        Ok(())
    }

    fn set_text(&self, oculos_id: &str, text: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_ValuePatternId)
                .context("Element does not support the ValuePattern")?;
            let vp: IUIAutomationValuePattern = windows::core::Interface::cast(&raw)?;
            vp.SetValue(&BSTR::from(text))?;
        }
        Ok(())
    }

    fn send_keys(&self, oculos_id: &str, text: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe { elem.SetFocus()? };
        std::thread::sleep(std::time::Duration::from_millis(60));
        send_key_sequence(text);
        Ok(())
    }

    fn focus_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe { elem.SetFocus()? };
        Ok(())
    }

    // ── Pattern-specific interactions ──────────────────────────────────────

    fn toggle_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_TogglePatternId)
                .context("Element does not support TogglePattern")?;
            let tp: IUIAutomationTogglePattern = windows::core::Interface::cast(&raw)?;
            tp.Toggle()?;
        }
        Ok(())
    }

    fn expand_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_ExpandCollapsePatternId)
                .context("Element does not support ExpandCollapsePattern")?;
            let ep: IUIAutomationExpandCollapsePattern = windows::core::Interface::cast(&raw)?;
            ep.Expand()?;
        }
        Ok(())
    }

    fn collapse_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_ExpandCollapsePatternId)
                .context("Element does not support ExpandCollapsePattern")?;
            let ep: IUIAutomationExpandCollapsePattern = windows::core::Interface::cast(&raw)?;
            ep.Collapse()?;
        }
        Ok(())
    }

    fn select_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_SelectionItemPatternId)
                .context("Element does not support SelectionItemPattern")?;
            let sp: IUIAutomationSelectionItemPattern = windows::core::Interface::cast(&raw)?;
            sp.Select()?;
        }
        Ok(())
    }

    fn set_range(&self, oculos_id: &str, value: f64) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_RangeValuePatternId)
                .context("Element does not support RangeValuePattern")?;
            let rp: IUIAutomationRangeValuePattern = windows::core::Interface::cast(&raw)?;
            rp.SetValue(value)?;
        }
        Ok(())
    }

    fn scroll_element(&self, oculos_id: &str, direction: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_ScrollPatternId)
                .context("Element does not support ScrollPattern")?;
            let sp: IUIAutomationScrollPattern = windows::core::Interface::cast(&raw)?;
            let (h, v) = match direction {
                "up" => (ScrollAmount_NoAmount, ScrollAmount_SmallDecrement),
                "down" => (ScrollAmount_NoAmount, ScrollAmount_SmallIncrement),
                "left" => (ScrollAmount_SmallDecrement, ScrollAmount_NoAmount),
                "right" => (ScrollAmount_SmallIncrement, ScrollAmount_NoAmount),
                "page-up" => (ScrollAmount_NoAmount, ScrollAmount_LargeDecrement),
                "page-down" => (ScrollAmount_NoAmount, ScrollAmount_LargeIncrement),
                other => {
                    return Err(anyhow!(
                    "Unknown scroll direction '{}'. Use: up, down, left, right, page-up, page-down",
                    other
                ))
                }
            };
            sp.Scroll(h, v)?;
        }
        Ok(())
    }

    fn scroll_into_view(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_element(oculos_id)?;
        unsafe {
            let raw = elem
                .GetCurrentPattern(UIA_ScrollItemPatternId)
                .context("Element does not support ScrollItemPattern")?;
            let sp: IUIAutomationScrollItemPattern = windows::core::Interface::cast(&raw)?;
            sp.ScrollIntoView()?;
        }
        Ok(())
    }

    // ── Window operations ──────────────────────────────────────────────────

    fn focus_window(&self, pid: u32) -> Result<()> {
        let hwnd = find_main_window(pid)
            .ok_or_else(|| anyhow!("No visible window found for PID {}", pid))?;
        unsafe { SetForegroundWindow(hwnd).ok() };
        Ok(())
    }

    fn close_window(&self, pid: u32) -> Result<()> {
        let hwnd = find_main_window(pid)
            .ok_or_else(|| anyhow!("No visible window found for PID {}", pid))?;
        unsafe {
            let elem = self
                .automation
                .ElementFromHandle(hwnd)
                .context("ElementFromHandle failed")?;
            let raw = elem
                .GetCurrentPattern(UIA_WindowPatternId)
                .context("Window does not support WindowPattern")?;
            let wp: IUIAutomationWindowPattern = windows::core::Interface::cast(&raw)?;
            wp.Close()?;
        }
        Ok(())
    }

    fn highlight_element(&self, oculos_id: &str, duration_ms: u64) -> Result<Rect> {
        let elem = self.find_element(oculos_id)?;
        let r = unsafe { elem.CurrentBoundingRectangle()? };

        let rect = Rect {
            x: r.left,
            y: r.top,
            width: r.right - r.left,
            height: r.bottom - r.top,
        };

        // Draw on a background thread so we don't block the API response
        let x = r.left;
        let y = r.top;
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        let dur = duration_ms.min(5000); // cap at 5 seconds

        std::thread::spawn(move || unsafe {
            draw_highlight_rect(x, y, w, h, dur);
        });

        Ok(rect)
    }

    fn screenshot_window(&self, pid: u32) -> Result<Vec<u8>> {
        let hwnd = find_main_window(pid)
            .ok_or_else(|| anyhow!("No visible window found for PID {}", pid))?;

        unsafe {
            let mut rc = RECT::default();
            GetWindowRect(hwnd, &mut rc)?;
            let w = (rc.right - rc.left) as i32;
            let h = (rc.bottom - rc.top) as i32;
            if w <= 0 || h <= 0 {
                return Err(anyhow!("Window has zero or negative size"));
            }

            let hdc_screen = GetDC(HWND::default());
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbmp = CreateCompatibleBitmap(hdc_screen, w, h);
            let old = SelectObject(hdc_mem, hbmp);

            BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, rc.left, rc.top, SRCCOPY)?;

            // Read pixels via GetDIBits
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0 as u32,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut pixels = vec![0u8; (w * h * 4) as usize];
            GetDIBits(
                hdc_mem,
                hbmp,
                0,
                h as u32,
                Some(pixels.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // Cleanup GDI
            SelectObject(hdc_mem, old);
            DeleteObject(hbmp);
            DeleteDC(hdc_mem);
            ReleaseDC(HWND::default(), hdc_screen);

            // Convert BGRA → RGBA
            for chunk in pixels.chunks_exact_mut(4) {
                chunk.swap(0, 2); // B ↔ R
            }

            // Encode to PNG
            let img = image::RgbaImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| anyhow!("Failed to create image buffer"))?;
            let mut png_buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png_buf, image::ImageFormat::Png)
                .context("Failed to encode PNG")?;

            Ok(png_buf.into_inner())
        }
    }
}

// ── Win32 helpers ─────────────────────────────────────────────────────────────

unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HWND>);
    list.push(hwnd);
    BOOL(1)
}

// ── Keyboard engine ───────────────────────────────────────────────────────────
//
// Syntax supported in send_keys text:
//   {ENTER}  {TAB}  {ESC}  {BACKSPACE}  {DELETE}
//   {HOME}   {END}  {LEFT} {RIGHT} {UP} {DOWN}
//   {CTRL+A} {CTRL+C} {CTRL+V} {CTRL+X} {CTRL+Z} {CTRL+L} {CTRL+W}
//   {ALT+F4} {WIN}  {F1}..{F12}
//   Any other character → Unicode keypress
//
fn send_key_sequence(text: &str) {
    let mut iter = text.chars().peekable();
    while let Some(ch) = iter.next() {
        if ch == '{' {
            let mut name = String::new();
            for c in iter.by_ref() {
                if c == '}' {
                    break;
                }
                name.push(c);
            }
            dispatch_special(&name.to_uppercase());
        } else {
            send_unicode(ch);
        }
    }
}

fn dispatch_special(name: &str) {
    match name {
        "ENTER" | "RETURN" => send_vk(0x0D),
        "TAB" => send_vk(0x09),
        "ESC" | "ESCAPE" => send_vk(0x1B),
        "BACKSPACE" | "BS" => send_vk(0x08),
        "DELETE" | "DEL" => send_vk(0x2E),
        "HOME" => send_vk(0x24),
        "END" => send_vk(0x23),
        "LEFT" => send_vk(0x25),
        "RIGHT" => send_vk(0x27),
        "UP" => send_vk(0x26),
        "DOWN" => send_vk(0x28),
        "PGUP" => send_vk(0x21),
        "PGDN" => send_vk(0x22),
        "WIN" => send_vk(0x5B),
        "F1" => send_vk(0x70),
        "F2" => send_vk(0x71),
        "F3" => send_vk(0x72),
        "F4" => send_vk(0x73),
        "F5" => send_vk(0x74),
        "F6" => send_vk(0x75),
        "F7" => send_vk(0x76),
        "F8" => send_vk(0x77),
        "F9" => send_vk(0x78),
        "F10" => send_vk(0x79),
        "F11" => send_vk(0x7A),
        "F12" => send_vk(0x7B),
        // Modifier combos  ── CTRL+key
        chord if chord.starts_with("CTRL+") => {
            let key = chord.trim_start_matches("CTRL+");
            let vk = char_to_vk(key);
            send_chord(&[0x11, vk]); // VK_CONTROL + key
        }
        // ALT+key
        chord if chord.starts_with("ALT+") => {
            let key = chord.trim_start_matches("ALT+");
            let vk = char_to_vk(key);
            send_chord(&[0x12, vk]); // VK_MENU + key
        }
        // SHIFT+key
        chord if chord.starts_with("SHIFT+") => {
            let key = chord.trim_start_matches("SHIFT+");
            let vk = char_to_vk(key);
            send_chord(&[0x10, vk]); // VK_SHIFT + key
        }
        _ => {} // unknown special key — skip
    }
}

/// Single character → virtual key code (for use in chords)
fn char_to_vk(s: &str) -> u16 {
    if s.len() == 1 {
        let c = s.chars().next().unwrap().to_ascii_uppercase();
        return c as u16; // A–Z and 0–9 map directly to VK codes
    }
    match s {
        "F4" => 0x73,
        "F5" => 0x74,
        _ => 0x00,
    }
}

/// Send a virtual key down + up (for special keys, no unicode flag).
unsafe fn send_vk_raw(vk: u16, key_up: bool) {
    let flags = if key_up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    SendInput(&[input], size_of::<INPUT>() as i32);
}

fn send_vk(vk: u16) {
    unsafe {
        send_vk_raw(vk, false);
        send_vk_raw(vk, true);
    }
}

/// Send a modifier + key chord (e.g. Ctrl+A).
fn send_chord(vks: &[u16]) {
    unsafe {
        for &vk in vks {
            send_vk_raw(vk, false);
        } // press all down
        for &vk in vks.iter().rev() {
            send_vk_raw(vk, true);
        } // release in reverse
    }
}

/// Send a single Unicode character via KEYEVENTF_UNICODE.
fn send_unicode(ch: char) {
    // Characters outside BMP need surrogate pairs — handle them correctly
    let mut buf = [0u16; 2];
    let encoded = ch.encode_utf16(&mut buf);
    unsafe {
        for &scan in encoded.iter() {
            let down = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: scan,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            let up = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: scan,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            SendInput(&[down, up], size_of::<INPUT>() as i32);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

fn find_main_window(target_pid: u32) -> Option<HWND> {
    struct FindData {
        pid: u32,
        result: Option<HWND>,
    }
    let mut data = FindData {
        pid: target_pid,
        result: None,
    };
    let ptr = &mut data as *mut FindData as isize;

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let d = &mut *(lparam.0 as *mut FindData);
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == d.pid && IsWindowVisible(hwnd).as_bool() {
            let mut buf = [0u16; 8];
            if GetWindowTextW(hwnd, &mut buf) > 0 {
                d.result = Some(hwnd);
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(Some(callback), LPARAM(ptr));
    }
    data.result
}

/// Draw a highlight rectangle on the desktop using XOR pen.
/// Draws once, waits `duration_ms`, then draws again to erase (XOR cancels itself).
unsafe fn draw_highlight_rect(x: i32, y: i32, w: i32, h: i32, duration_ms: u64) {
    use windows::Win32::Graphics::Gdi::{GetStockObject, NULL_BRUSH};

    let thickness = 3i32;
    let color = windows::Win32::Foundation::COLORREF(0x00FF8D4C); // blue in BGR

    let pen = CreatePen(PS_SOLID, thickness, color);

    // --- Draw phase ---
    let hdc = GetDC(HWND::default());
    if hdc.is_invalid() {
        return;
    }

    let old_pen = SelectObject(hdc, pen);
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    SetROP2(hdc, R2_NOTXORPEN);

    windows::Win32::Graphics::Gdi::Rectangle(
        hdc,
        x - thickness,
        y - thickness,
        x + w + thickness,
        y + h + thickness,
    );

    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    ReleaseDC(HWND::default(), hdc);

    // --- Wait ---
    std::thread::sleep(std::time::Duration::from_millis(duration_ms));

    // --- Erase phase (XOR again cancels the drawing) ---
    let hdc = GetDC(HWND::default());
    if hdc.is_invalid() {
        let _ = DeleteObject(pen);
        return;
    }

    let old_pen = SelectObject(hdc, pen);
    let old_brush = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    SetROP2(hdc, R2_NOTXORPEN);

    windows::Win32::Graphics::Gdi::Rectangle(
        hdc,
        x - thickness,
        y - thickness,
        x + w + thickness,
        y + h + thickness,
    );

    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(pen);
    ReleaseDC(HWND::default(), hdc);
}

fn get_exe_name(pid: u32) -> String {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::from("unknown.exe");
        };
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .is_ok()
        {
            let path = String::from_utf16_lossy(&buf[..size as usize]);
            return path.split(['/', '\\']).last().unwrap_or("").to_string();
        }
        String::from("unknown.exe")
    }
}
