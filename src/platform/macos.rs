use anyhow::{anyhow, Context, Result};
use accessibility_sys::{
    kAXErrorAPIDisabled, kAXValueTypeCGPoint, kAXValueTypeCGSize, AXValueGetType,
    AXValueGetTypeID, AXValueGetValue, AXValueRef,
};
use dashmap::DashMap;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

use accessibility::{AXAttribute, AXUIElement};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use core_graphics::geometry::{CGPoint, CGSize};
use foreign_types::ForeignType;

use crate::{
    platform::UiBackend,
    types::{ElementType, ExpandState, RangeInfo, Rect, ToggleState, UiElement, WindowInfo},
};

// ── Element registry ──────────────────────────────────────────────────────────

struct SafeElement(AXUIElement);
unsafe impl Send for SafeElement {}
unsafe impl Sync for SafeElement {}

type IdRegistry = Arc<DashMap<String, SafeElement>>;

// ── Backend ───────────────────────────────────────────────────────────────────

pub struct MacOsUiBackend {
    registry: IdRegistry,
}

impl MacOsUiBackend {
    pub fn new() -> Result<Self> {
        // Check if we have accessibility permissions
        // AXIsProcessTrustedWithOptions is in ApplicationServices framework
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
        }
        let options = CFDictionary::from_CFType_pairs(&[(
            CFString::new("AXTrustedCheckOptionPrompt").as_CFType(),
            CFBoolean::true_value().as_CFType(),
        )]);
        let trusted = unsafe {
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as *const std::ffi::c_void)
        };
        if !trusted {
            tracing::warn!(
                "Accessibility permission not granted. \
                 Go to System Settings → Privacy & Security → Accessibility and enable \
                 the executable that launched OculOS (the shell/wrapper entry may appear \
                 instead of 'oculos-local')."
            );
        }
        Ok(Self {
            registry: Arc::new(DashMap::new()),
        })
    }

    // ── AX attribute helpers ──────────────────────────────────────────────

    fn get_string_attr(element: &AXUIElement, attr: &str) -> Option<String> {
        element
            .attribute(&AXAttribute::new(&CFString::new(attr)))
            .ok()
            .and_then(|v| v.downcast::<CFString>().map(|s| s.to_string()))
    }

    fn get_bool_attr(element: &AXUIElement, attr: &str) -> Option<bool> {
        element
            .attribute(&AXAttribute::new(&CFString::new(attr)))
            .ok()
            .and_then(|v| {
                v.downcast::<CFBoolean>()
                    .map(|b| b == CFBoolean::true_value())
            })
    }

    fn get_element_attr(element: &AXUIElement, attr: &str) -> Option<AXUIElement> {
        element
            .attribute(&AXAttribute::new(&CFString::new(attr)))
            .ok()
            .and_then(|v| v.downcast::<AXUIElement>())
    }

    fn get_element_array_attr(element: &AXUIElement, attr: &str) -> Vec<AXUIElement> {
        element
            .attribute(&AXAttribute::new(&CFString::new(attr)))
            .ok()
            .and_then(|v| v.downcast::<CFArray>())
            .map(|arr| {
                arr.get_all_values()
                    .into_iter()
                    .map(|item| unsafe { AXUIElement::wrap_under_get_rule(item as _) })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn push_unique_element(
        elements: &mut Vec<AXUIElement>,
        seen: &mut HashSet<usize>,
        candidate: Option<AXUIElement>,
    ) {
        if let Some(element) = candidate {
            let key = element.as_CFTypeRef() as usize;
            if seen.insert(key) {
                elements.push(element);
            }
        }
    }

    fn get_number_attr(element: &AXUIElement, attr: &str) -> Option<f64> {
        element
            .attribute(&AXAttribute::new(&CFString::new(attr)))
            .ok()
            .and_then(|v| v.downcast::<CFNumber>().and_then(|n| n.to_f64()))
    }

    fn get_children(element: &AXUIElement) -> Vec<AXUIElement> {
        let mut children = Vec::new();
        let mut seen = HashSet::new();

        for child in Self::get_element_array_attr(element, "AXVisibleChildren") {
            Self::push_unique_element(&mut children, &mut seen, Some(child));
        }
        for child in Self::get_element_array_attr(element, "AXChildren") {
            Self::push_unique_element(&mut children, &mut seen, Some(child));
        }
        Self::push_unique_element(
            &mut children,
            &mut seen,
            Self::get_element_attr(element, "AXContents"),
        );

        children
    }

    fn first_string_attr(element: &AXUIElement, attrs: &[&str]) -> Option<String> {
        attrs
            .iter()
            .find_map(|attr| Self::get_string_attr(element, attr))
            .filter(|value| !value.trim().is_empty())
    }

    fn get_text_content(element: &AXUIElement, role: &str) -> Option<String> {
        Self::first_string_attr(
            element,
            &[
                "AXValue",
                "AXPlaceholderValue",
                "AXDescription",
                "AXTitle",
                "AXLabel",
                "AXHelp",
                "AXURL",
            ],
        )
        .filter(|value| role == "AXWebArea" || value.len() <= 2048)
    }

    fn is_likely_text_input(role: &str, subrole: Option<&str>) -> bool {
        matches!(
            role,
            "AXTextField" | "AXTextArea" | "AXSecureTextField" | "AXComboBox"
        ) || matches!(subrole, Some("AXSearchField" | "AXTextArea"))
    }

    fn is_likely_scrollable(role: &str) -> bool {
        matches!(
            role,
            "AXScrollArea" | "AXWebArea" | "AXTable" | "AXList" | "AXOutline"
        )
    }

    // ── Role → ElementType mapping ────────────────────────────────────────

    fn role_to_element_type(role: &str) -> ElementType {
        match role {
            "AXWindow" => ElementType::Window,
            "AXButton" => ElementType::Button,
            "AXTextField" | "AXTextArea" | "AXSecureTextField" => ElementType::Edit,
            "AXStaticText" => ElementType::Text,
            "AXCheckBox" => ElementType::CheckBox,
            "AXRadioButton" => ElementType::RadioButton,
            "AXComboBox" | "AXPopUpButton" => ElementType::ComboBox,
            "AXList" => ElementType::ListBox,
            "AXRow" | "AXCell" => ElementType::ListItem,
            "AXOutline" => ElementType::TreeView,
            "AXOutlineRow" => ElementType::TreeItem,
            "AXMenu" | "AXMenuBar" => ElementType::Menu,
            "AXMenuItem" => ElementType::MenuItem,
            "AXTabGroup" => ElementType::TabControl,
            "AXRadioGroup" => ElementType::Group,
            "AXToolbar" => ElementType::ToolBar,
            "AXScrollBar" => ElementType::ScrollBar,
            "AXSlider" => ElementType::Slider,
            "AXProgressIndicator" => ElementType::ProgressBar,
            "AXImage" => ElementType::Image,
            "AXLink" => ElementType::Link,
            "AXGroup" | "AXSplitGroup" | "AXLayoutArea" => ElementType::Group,
            "AXScrollArea" => ElementType::Pane,
            "AXSheet" | "AXDialog" => ElementType::Dialog,
            "AXWebArea" => ElementType::Document,
            "AXTable" => ElementType::Table,
            _ => ElementType::Unknown,
        }
    }

    // ── Actions discovery ─────────────────────────────────────────────────

    fn collect_actions(element: &AXUIElement) -> Vec<String> {
        let ax_actions = element
            .action_names()
            .ok()
            .map(|arr| {
                (0..arr.len())
                    .filter_map(|i| arr.get(i).map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut actions = Vec::new();
        for ax in &ax_actions {
            match ax.as_str() {
                "AXPress" => actions.push("click".into()),
                "AXConfirm" => {
                    if !actions.contains(&"click".to_string()) {
                        actions.push("click".into());
                    }
                }
                "AXShowMenu" => actions.push("expand".into()),
                "AXCancel" => actions.push("collapse".into()),
                "AXPick" => actions.push("select".into()),
                _ => {}
            }
        }

        let role = Self::get_string_attr(element, "AXRole").unwrap_or_default();
        let subrole = Self::get_string_attr(element, "AXSubrole");

        // Check if element is settable (for set-text)
        if Self::get_bool_attr(element, "AXSettable:AXValue").unwrap_or(false) {
            match role.as_str() {
                "AXTextField" | "AXTextArea" | "AXSecureTextField" | "AXComboBox" => {
                    actions.push("set-text".into());
                    actions.push("send-keys".into());
                }
                "AXSlider" => {
                    actions.push("set-range".into());
                }
                "AXCheckBox" => {
                    if !actions.contains(&"click".to_string()) {
                        actions.push("toggle".into());
                    }
                }
                _ => {}
            }
        }

        if Self::is_likely_text_input(&role, subrole.as_deref()) {
            if !actions.iter().any(|action| action == "set-text") {
                actions.push("set-text".into());
            }
            if !actions.iter().any(|action| action == "send-keys") {
                actions.push("send-keys".into());
            }
        }

        if Self::is_likely_scrollable(&role) && !actions.iter().any(|action| action == "scroll") {
            actions.push("scroll".into());
        }

        // Focus
        if Self::get_bool_attr(element, "AXFocused").is_some() {
            actions.push("focus".into());
        }

        actions
    }

    // ── Build element ─────────────────────────────────────────────────────

    fn build_element(
        &self,
        element: &AXUIElement,
        with_children: bool,
        depth: u32,
    ) -> Result<UiElement> {
        if depth > 48 {
            let id = Uuid::new_v4().to_string();
            self.registry
                .insert(id.clone(), SafeElement(element.clone()));
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

        let role = Self::get_string_attr(element, "AXRole").unwrap_or_default();
        let element_type = Self::role_to_element_type(&role);

        let subrole = Self::get_string_attr(element, "AXSubrole");
        let role_description = Self::get_string_attr(element, "AXRoleDescription");

        let label = Self::first_string_attr(
            element,
            &[
                "AXTitle",
                "AXDescription",
                "AXLabel",
                "AXHelp",
                "AXPlaceholderValue",
                "AXIdentifier",
                "AXRoleDescription",
            ],
        )
        .unwrap_or_default();

        let value = Self::first_string_attr(element, &["AXValue", "AXURL"]);
        let text_content = Self::get_text_content(element, &role);

        let enabled = Self::get_bool_attr(element, "AXEnabled").unwrap_or(true);
        let focused = Self::get_bool_attr(element, "AXFocused").unwrap_or(false);
        let is_keyboard_focusable = Self::get_bool_attr(element, "AXFocused").is_some();

        // Position & size
        let (x, y) = Self::get_position(element);
        let (width, height) = Self::get_size(element);
        let rect = Rect {
            x,
            y,
            width,
            height,
        };

        // Toggle state (for checkboxes)
        let toggle_state = if element_type == ElementType::CheckBox {
            Self::get_number_attr(element, "AXValue").map(|v| {
                if v == 1.0 {
                    ToggleState::On
                } else if v == 0.0 {
                    ToggleState::Off
                } else {
                    ToggleState::Indeterminate
                }
            })
        } else {
            None
        };

        // Selection state
        let is_selected = Self::get_bool_attr(element, "AXSelected");

        // Expand state
        let expand_state = Self::get_bool_attr(element, "AXExpanded").map(|expanded| {
            if expanded {
                ExpandState::Expanded
            } else {
                ExpandState::Collapsed
            }
        });

        // Range info (sliders)
        let range = if element_type == ElementType::Slider {
            Some(RangeInfo {
                value: Self::get_number_attr(element, "AXValue").unwrap_or(0.0),
                minimum: Self::get_number_attr(element, "AXMinValue").unwrap_or(0.0),
                maximum: Self::get_number_attr(element, "AXMaxValue").unwrap_or(100.0),
                step: 1.0,
                read_only: !Self::get_bool_attr(element, "AXSettable:AXValue").unwrap_or(false),
            })
        } else {
            None
        };

        let automation_id = Self::first_string_attr(element, &["AXIdentifier", "AXDOMIdentifier"]);
        let help_text = Self::get_string_attr(element, "AXHelp");
        let class_name = subrole.or(role_description);

        let actions = Self::collect_actions(element);

        // Children
        let children = if with_children {
            Self::get_children(element)
                .iter()
                .filter_map(|child| self.build_element(child, true, depth + 1).ok())
                .collect()
        } else {
            vec![]
        };

        let oculos_id = Uuid::new_v4().to_string();
        self.registry
            .insert(oculos_id.clone(), SafeElement(element.clone()));

        Ok(UiElement {
            oculos_id,
            element_type,
            label,
            value,
            text_content,
            rect,
            enabled,
            focused,
            is_keyboard_focusable,
            toggle_state,
            is_selected,
            expand_state,
            range,
            automation_id,
            class_name,
            help_text,
            keyboard_shortcut: None,
            actions,
            children,
        })
    }

    // ── Position / Size helpers ───────────────────────────────────────────

    fn get_position(element: &AXUIElement) -> (i32, i32) {
        let Some(value) = element
            .attribute(&AXAttribute::new(&CFString::new("AXPosition")))
            .ok()
        else {
            return (0, 0);
        };

        if unsafe { core_foundation::base::CFGetTypeID(value.as_CFTypeRef()) } != unsafe {
            AXValueGetTypeID()
        } {
            return (0, 0);
        }

        let ax_value = value.as_CFTypeRef() as AXValueRef;
        if unsafe { AXValueGetType(ax_value) } != kAXValueTypeCGPoint {
            return (0, 0);
        }

        let mut point = CGPoint::new(0.0, 0.0);
        if unsafe {
            AXValueGetValue(
                ax_value,
                kAXValueTypeCGPoint,
                &mut point as *mut CGPoint as *mut std::ffi::c_void,
            )
        } {
            (point.x as i32, point.y as i32)
        } else {
            (0, 0)
        }
    }

    fn get_size(element: &AXUIElement) -> (i32, i32) {
        let Some(value) = element.attribute(&AXAttribute::new(&CFString::new("AXSize"))).ok()
        else {
            return (0, 0);
        };

        if unsafe { core_foundation::base::CFGetTypeID(value.as_CFTypeRef()) } != unsafe {
            AXValueGetTypeID()
        } {
            return (0, 0);
        }

        let ax_value = value.as_CFTypeRef() as AXValueRef;
        if unsafe { AXValueGetType(ax_value) } != kAXValueTypeCGSize {
            return (0, 0);
        }

        let mut size = CGSize::new(0.0, 0.0);
        if unsafe {
            AXValueGetValue(
                ax_value,
                kAXValueTypeCGSize,
                &mut size as *mut CGSize as *mut std::ffi::c_void,
            )
        } {
            (size.width as i32, size.height as i32)
        } else {
            (0, 0)
        }
    }

    // ── Get app element for PID ───────────────────────────────────────────

    fn app_element(pid: u32) -> AXUIElement {
        AXUIElement::application(pid as i32)
    }

    fn ensure_accessibility_ready(&self, pid: u32, app: &AXUIElement) -> Result<()> {
        app.attribute_names().map(|_| ()).map_err(|err| {
            if err == kAXErrorAPIDisabled {
                anyhow!(
                "Accessibility API disabled for PID {}. Grant Accessibility to the executable \
                 that launched OculOS; on macOS this may appear as your shell or wrapper rather \
                 than 'oculos-local'.",
                pid
            )
            } else {
                anyhow!(
                    "Failed to access macOS accessibility tree for PID {} (AXError {}).",
                    pid,
                    err
                )
            }
        })
    }

    fn window_roots(app: &AXUIElement) -> Vec<AXUIElement> {
        let mut roots = Vec::new();
        let mut seen = HashSet::new();

        Self::push_unique_element(
            &mut roots,
            &mut seen,
            Self::get_element_attr(app, "AXFocusedWindow"),
        );
        Self::push_unique_element(
            &mut roots,
            &mut seen,
            Self::get_element_attr(app, "AXMainWindow"),
        );
        for window in Self::get_element_array_attr(app, "AXWindows") {
            Self::push_unique_element(&mut roots, &mut seen, Some(window));
        }

        if roots.is_empty() {
            for child in Self::get_children(app) {
                let role = Self::get_string_attr(&child, "AXRole").unwrap_or_default();
                if matches!(role.as_str(), "AXWindow" | "AXSheet" | "AXDialog") {
                    Self::push_unique_element(&mut roots, &mut seen, Some(child));
                }
            }
        }

        roots
    }

    // ── Registry lookup ───────────────────────────────────────────────────

    fn find_registered(&self, oculos_id: &str) -> Result<AXUIElement> {
        let entry = self.registry.get(oculos_id).ok_or_else(|| {
            anyhow!(
                "Element '{}' not found — refresh via GET /windows/{{pid}}/tree first.",
                oculos_id
            )
        })?;
        Ok(entry.value().0.clone())
    }

    // ── Flat search ───────────────────────────────────────────────────────

    fn search_elements(
        &self,
        root: &AXUIElement,
        query: Option<&str>,
        element_type: Option<&ElementType>,
        interactive_only: bool,
        results: &mut Vec<UiElement>,
        depth: u32,
    ) {
        if depth > 48 || results.len() >= 500 {
            return;
        }

        if let Ok(elem) = self.build_element(root, false, depth) {
            let query_lower = query.map(|q| q.to_lowercase());
            let mut matches = true;

            if let Some(ref q) = query_lower {
                let label_match = elem.label.to_lowercase().contains(q.as_str());
                let value_match = elem
                    .value
                    .as_ref()
                    .map(|value| value.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                let text_match = elem
                    .text_content
                    .as_ref()
                    .map(|text| text.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                let help_match = elem
                    .help_text
                    .as_ref()
                    .map(|text| text.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                let class_match = elem
                    .class_name
                    .as_ref()
                    .map(|text| text.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                let aid_match = elem
                    .automation_id
                    .as_ref()
                    .map(|a| a.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                if !label_match
                    && !aid_match
                    && !value_match
                    && !text_match
                    && !help_match
                    && !class_match
                {
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

        for child in Self::get_children(root) {
            self.search_elements(
                &child,
                query,
                element_type,
                interactive_only,
                results,
                depth + 1,
            );
        }
    }
}

// ── UiBackend implementation ──────────────────────────────────────────────────

impl UiBackend for MacOsUiBackend {
    fn list_windows(&self) -> Result<Vec<WindowInfo>> {
        use core_foundation::base::{CFTypeRef, TCFType};
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::dictionary::CFDictionaryRef;
        use core_foundation::number::CFNumber;
        use core_foundation::string::CFString;
        use core_graphics::window::{
            kCGNullWindowID, kCGWindowBounds, kCGWindowLayer, kCGWindowListExcludeDesktopElements,
            kCGWindowListOptionOnScreenOnly, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName,
            kCGWindowOwnerPID, CGWindowListCopyWindowInfo,
        };

        let mut windows = Vec::new();

        let option = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let array_ref = unsafe { CGWindowListCopyWindowInfo(option, kCGNullWindowID) };
        if array_ref.is_null() {
            return Err(anyhow!("CGWindowListCopyWindowInfo returned null"));
        }

        let count = unsafe { core_foundation::array::CFArrayGetCount(array_ref) };

        for i in 0..count {
            unsafe {
                let dict_ref =
                    core_foundation::array::CFArrayGetValueAtIndex(array_ref, i) as CFDictionaryRef;
                if dict_ref.is_null() {
                    continue;
                }

                // Helper to get a value from the dict by key
                let get_val = |key: CFTypeRef| -> CFTypeRef {
                    let mut value: CFTypeRef = std::ptr::null();
                    if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
                        dict_ref,
                        key as *const _,
                        &mut value as *mut _ as *mut _,
                    ) != 0
                    {
                        value
                    } else {
                        std::ptr::null()
                    }
                };

                // Extract PID
                let pid_val = get_val(kCGWindowOwnerPID as CFTypeRef);
                if pid_val.is_null() {
                    continue;
                }
                let pid_num = CFNumber::wrap_under_get_rule(pid_val as _);
                let pid = pid_num.to_i64().unwrap_or(0) as u32;
                if pid == 0 {
                    continue;
                }

                // Extract layer — skip windows on non-standard layers
                let layer_val = get_val(kCGWindowLayer as CFTypeRef);
                let layer = if !layer_val.is_null() {
                    CFNumber::wrap_under_get_rule(layer_val as _)
                        .to_i64()
                        .unwrap_or(0)
                } else {
                    0
                };
                if layer != 0 {
                    continue;
                }

                // Extract owner name
                let name_val = get_val(kCGWindowOwnerName as CFTypeRef);
                let owner_name = if !name_val.is_null() {
                    CFString::wrap_under_get_rule(name_val as _).to_string()
                } else {
                    String::new()
                };
                if owner_name.is_empty() {
                    continue;
                }

                // Extract window name
                let wname_val = get_val(kCGWindowName as CFTypeRef);
                let window_name = if !wname_val.is_null() {
                    CFString::wrap_under_get_rule(wname_val as _).to_string()
                } else {
                    String::new()
                };

                let title = if window_name.is_empty() {
                    owner_name.clone()
                } else {
                    window_name
                };

                // Extract window number as pseudo-hwnd
                let num_val = get_val(kCGWindowNumber as CFTypeRef);
                let window_number = if !num_val.is_null() {
                    CFNumber::wrap_under_get_rule(num_val as _)
                        .to_i64()
                        .unwrap_or(0) as usize
                } else {
                    0
                };

                let mut rect = Rect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                };

                let bounds_value = get_val(kCGWindowBounds as CFTypeRef);
                if !bounds_value.is_null() {
                    let bounds_dict =
                        CFDictionary::<CFString, CFType>::wrap_under_get_rule(bounds_value as _);
                    let get_bound = |key: &str| {
                        bounds_dict
                            .find(&CFString::new(key))
                            .and_then(|v| v.downcast::<CFNumber>().and_then(|n| n.to_i64()))
                            .unwrap_or(0) as i32
                    };
                    rect = Rect {
                        x: get_bound("X"),
                        y: get_bound("Y"),
                        width: get_bound("Width"),
                        height: get_bound("Height"),
                    };
                }

                windows.push(WindowInfo {
                    pid,
                    hwnd: window_number,
                    title,
                    exe_name: owner_name,
                    rect,
                    visible: true,
                });
            }
        }

        // Release the array
        unsafe { core_foundation::base::CFRelease(array_ref as _) };

        Ok(windows)
    }

    fn get_ui_tree(&self, pid: u32) -> Result<UiElement> {
        let app = Self::app_element(pid);
        self.ensure_accessibility_ready(pid, &app)?;

        let root = Self::window_roots(&app).into_iter().next().unwrap_or(app);
        self.build_element(&root, true, 0)
    }

    fn get_ui_tree_hwnd(&self, _hwnd: usize) -> Result<UiElement> {
        // On macOS, hwnd is not used in the same way. We treat it as a window index.
        // The caller should use pid-based access instead.
        Err(anyhow!(
            "macOS does not use window handles. Use the PID-based endpoint instead."
        ))
    }

    fn find_elements(
        &self,
        pid: u32,
        query: Option<&str>,
        element_type: Option<&ElementType>,
        interactive_only: bool,
    ) -> Result<Vec<UiElement>> {
        let app = Self::app_element(pid);
        self.ensure_accessibility_ready(pid, &app)?;
        let mut results = Vec::new();
        let roots = Self::window_roots(&app);
        if roots.is_empty() {
            self.search_elements(&app, query, element_type, interactive_only, &mut results, 0);
        } else {
            for root in roots {
                self.search_elements(
                    &root,
                    query,
                    element_type,
                    interactive_only,
                    &mut results,
                    0,
                );
            }
        }
        Ok(results)
    }

    fn find_elements_hwnd(
        &self,
        _hwnd: usize,
        _query: Option<&str>,
        _element_type: Option<&ElementType>,
        _interactive_only: bool,
    ) -> Result<Vec<UiElement>> {
        Err(anyhow!(
            "macOS does not use window handles. Use the PID-based endpoint instead."
        ))
    }

    fn click_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;
        elem.perform_action(&CFString::new("AXPress"))
            .map_err(|_| anyhow!("AXPress action failed on element '{}'", oculos_id))
    }

    fn set_text(&self, oculos_id: &str, text: &str) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;
        elem.set_attribute(
            &AXAttribute::new(&CFString::new("AXValue")),
            CFString::new(text).as_CFType(),
        )
        .map_err(|_| anyhow!("Failed to set AXValue on element '{}'", oculos_id))
    }

    fn send_keys(&self, oculos_id: &str, text: &str) -> Result<()> {
        // Focus the element first
        self.focus_element(oculos_id)?;
        std::thread::sleep(std::time::Duration::from_millis(60));

        // Use CGEvent to simulate keyboard input
        send_key_sequence_macos(text);
        Ok(())
    }

    fn focus_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;
        elem.set_attribute(
            &AXAttribute::new(&CFString::new("AXFocused")),
            CFBoolean::true_value().as_CFType(),
        )
        .map_err(|_| anyhow!("Failed to focus element '{}'", oculos_id))
    }

    fn toggle_element(&self, oculos_id: &str) -> Result<()> {
        // On macOS, toggling a checkbox is done via AXPress
        self.click_element(oculos_id)
    }

    fn expand_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;
        elem.perform_action(&CFString::new("AXShowMenu"))
            .or_else(|_| elem.perform_action(&CFString::new("AXPress")))
            .map_err(|_| anyhow!("Failed to expand element '{}'", oculos_id))
    }

    fn collapse_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;
        elem.perform_action(&CFString::new("AXCancel"))
            .or_else(|_| elem.perform_action(&CFString::new("AXPress")))
            .map_err(|_| anyhow!("Failed to collapse element '{}'", oculos_id))
    }

    fn select_element(&self, oculos_id: &str) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;
        elem.perform_action(&CFString::new("AXPick"))
            .or_else(|_| elem.perform_action(&CFString::new("AXPress")))
            .map_err(|_| anyhow!("Failed to select element '{}'", oculos_id))
    }

    fn set_range(&self, oculos_id: &str, value: f64) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;
        let cf_value = CFNumber::from(value);
        elem.set_attribute(
            &AXAttribute::new(&CFString::new("AXValue")),
            cf_value.as_CFType(),
        )
        .map_err(|_| anyhow!("Failed to set range value on element '{}'", oculos_id))
    }

    fn scroll_element(&self, oculos_id: &str, direction: &str) -> Result<()> {
        let elem = self.find_registered(oculos_id)?;

        // macOS scrolling is done via CGEvent scroll wheel events
        let (dx, dy): (i32, i32) = match direction {
            "up" => (0, 3),
            "down" => (0, -3),
            "left" => (3, 0),
            "right" => (-3, 0),
            "page-up" => (0, 10),
            "page-down" => (0, -10),
            other => {
                return Err(anyhow!(
                    "Unknown scroll direction '{}'. Use: up, down, left, right, page-up, page-down",
                    other
                ))
            }
        };

        let (x, y) = Self::get_position(&elem);
        let (width, height) = Self::get_size(&elem);
        if width > 0 && height > 0 {
            let center = CGPoint::new((x + width / 2) as f64, (y + height / 2) as f64);
            core_graphics::display::CGDisplay::warp_mouse_cursor_position(center)
                .map_err(|_| anyhow!("Failed to move cursor to element '{}'", oculos_id))?;
            std::thread::sleep(std::time::Duration::from_millis(40));
        }

        let _ = elem.set_attribute(
            &AXAttribute::new(&CFString::new("AXFocused")),
            CFBoolean::true_value().as_CFType(),
        );

        use core_graphics::event::{CGEvent, ScrollEventUnit};
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| anyhow!("Failed to create CGEventSource"))?;
        let event = CGEvent::new_scroll_event(source, ScrollEventUnit::LINE, 2, dy, dx, 0)
            .map_err(|_| anyhow!("Failed to create scroll event"))?;
        event.post(core_graphics::event::CGEventTapLocation::HID);

        Ok(())
    }

    fn scroll_into_view(&self, oculos_id: &str) -> Result<()> {
        // macOS accessibility doesn't have a direct scroll-into-view action.
        // We attempt to use AXScrollToVisible if available.
        let elem = self.find_registered(oculos_id)?;
        elem.perform_action(&CFString::new("AXScrollToVisible"))
            .map_err(|_| {
                anyhow!(
                    "AXScrollToVisible not supported on element '{}'. \
                     Try scrolling the parent container manually.",
                    oculos_id
                )
            })
    }

    fn focus_window(&self, pid: u32) -> Result<()> {
        let app = Self::app_element(pid);
        self.ensure_accessibility_ready(pid, &app)?;

        // Raise the application
        app.set_attribute(
            &AXAttribute::new(&CFString::new("AXFrontmost")),
            CFBoolean::true_value().as_CFType(),
        )
        .map_err(|_| anyhow!("Failed to bring PID {} to foreground", pid))?;

        // Also try to raise the first window
        let windows = Self::window_roots(&app);
        if let Some(win) = windows.first() {
            let _ = win.perform_action(&CFString::new("AXRaise"));
        }

        Ok(())
    }

    fn close_window(&self, pid: u32) -> Result<()> {
        let app = Self::app_element(pid);
        self.ensure_accessibility_ready(pid, &app)?;
        let windows = Self::window_roots(&app);

        if let Some(win) = windows.first() {
            // Find the close button
            let close_btn = win
                .attribute(&AXAttribute::new(&CFString::new("AXCloseButton")))
                .ok();

            if let Some(btn) = close_btn {
                if let Some(btn_elem) = btn.downcast::<AXUIElement>() {
                    btn_elem
                        .perform_action(&CFString::new("AXPress"))
                        .map_err(|_| anyhow!("Failed to click close button for PID {}", pid))?;
                    return Ok(());
                }
            }
        }

        Err(anyhow!("No closeable window found for PID {}", pid))
    }

    fn screenshot_window(&self, pid: u32) -> Result<Vec<u8>> {
        use core_graphics::display::CGDisplay;
        use core_graphics::geometry::{CGPoint, CGRect, CGSize};
        use core_graphics::window::{
            kCGWindowImageBoundsIgnoreFraming, kCGWindowListOptionIncludingWindow,
            CGWindowListCreateImage,
        };

        let window = self
            .list_windows()?
            .into_iter()
            .find(|window| window.pid == pid && window.rect.width > 0 && window.rect.height > 0)
            .ok_or_else(|| anyhow!("No visible window found for PID {}", pid))?;

        let rect = CGRect::new(
            &CGPoint::new(window.rect.x as f64, window.rect.y as f64),
            &CGSize::new(window.rect.width as f64, window.rect.height as f64),
        );

        let image = unsafe {
            CGWindowListCreateImage(
                rect,
                kCGWindowListOptionIncludingWindow,
                window.hwnd as u32,
                kCGWindowImageBoundsIgnoreFraming,
            )
        };

        let image = if image.is_null() {
            let display_image = CGDisplay::main()
                .image()
                .ok_or_else(|| anyhow!("Failed to capture window screenshot for PID {}", pid))?;
            display_image
        } else {
            unsafe { core_graphics::image::CGImage::from_ptr(image) }
        };

        encode_cgimage_to_png(&image)
    }
}

fn encode_cgimage_to_png(image: &core_graphics::image::CGImage) -> Result<Vec<u8>> {
    let width = image.width();
    let height = image.height();
    let bytes_per_row = image.bytes_per_row();
    let data = image.data();
    let src = data.bytes();

    if width == 0 || height == 0 || bytes_per_row == 0 {
        return Err(anyhow!("Screenshot image has invalid dimensions"));
    }

    let mut pixels = vec![0u8; width * height * 4];
    for y in 0..height {
        let row = &src[y * bytes_per_row..y * bytes_per_row + width * 4];
        for x in 0..width {
            let src_idx = x * 4;
            let dst_idx = (y * width + x) * 4;
            pixels[dst_idx] = row[src_idx + 1];
            pixels[dst_idx + 1] = row[src_idx + 2];
            pixels[dst_idx + 2] = row[src_idx + 3];
            pixels[dst_idx + 3] = row[src_idx];
        }
    }

    let img = image::RgbaImage::from_raw(width as u32, height as u32, pixels)
        .ok_or_else(|| anyhow!("Failed to create RGBA image buffer"))?;
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png)
        .context("Failed to encode PNG")?;
    Ok(png.into_inner())
}

// ── macOS keyboard simulation ─────────────────────────────────────────────────

fn send_key_sequence_macos(text: &str) {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Parse special key sequence
            let mut key_name = String::new();
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == '}' {
                    break;
                }
                key_name.push(c);
            }
            send_special_key_macos(&source, &key_name);
        } else {
            // Regular character — use CGEvent with unicode
            if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), 0, true) {
                event.set_string(&ch.to_string());
                event.post(core_graphics::event::CGEventTapLocation::HID);
            }
            if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), 0, false) {
                event.post(core_graphics::event::CGEventTapLocation::HID);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

fn send_special_key_macos(source: &core_graphics::event_source::CGEventSource, key_name: &str) {
    use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};

    let (keycode, flags): (CGKeyCode, CGEventFlags) = match key_name {
        "ENTER" | "RETURN" => (0x24, CGEventFlags::empty()),
        "TAB" => (0x30, CGEventFlags::empty()),
        "ESC" | "ESCAPE" => (0x35, CGEventFlags::empty()),
        "SPACE" => (0x31, CGEventFlags::empty()),
        "DELETE" => (0x75, CGEventFlags::empty()),
        "BACKSPACE" => (0x33, CGEventFlags::empty()),
        "UP" => (0x7E, CGEventFlags::empty()),
        "DOWN" => (0x7D, CGEventFlags::empty()),
        "LEFT" => (0x7B, CGEventFlags::empty()),
        "RIGHT" => (0x7C, CGEventFlags::empty()),
        "HOME" => (0x73, CGEventFlags::empty()),
        "END" => (0x77, CGEventFlags::empty()),
        "PGUP" => (0x74, CGEventFlags::empty()),
        "PGDN" => (0x79, CGEventFlags::empty()),
        "F1" => (0x7A, CGEventFlags::empty()),
        "F2" => (0x78, CGEventFlags::empty()),
        "F3" => (0x63, CGEventFlags::empty()),
        "F4" => (0x76, CGEventFlags::empty()),
        "F5" => (0x60, CGEventFlags::empty()),
        "F6" => (0x61, CGEventFlags::empty()),
        "F7" => (0x62, CGEventFlags::empty()),
        "F8" => (0x64, CGEventFlags::empty()),
        "F9" => (0x65, CGEventFlags::empty()),
        "F10" => (0x6D, CGEventFlags::empty()),
        "F11" => (0x67, CGEventFlags::empty()),
        "F12" => (0x6F, CGEventFlags::empty()),
        // Modifier combos: CTRL+X, CMD+X, ALT+X
        s if s.starts_with("CTRL+") || s.starts_with("CMD+") || s.starts_with("ALT+") => {
            let parts: Vec<&str> = s.splitn(2, '+').collect();
            let modifier = parts[0];
            let key_char = parts.get(1).unwrap_or(&"").to_lowercase();

            let flag = match modifier {
                "CTRL" => CGEventFlags::CGEventFlagControl,
                "CMD" => CGEventFlags::CGEventFlagCommand,
                "ALT" => CGEventFlags::CGEventFlagAlternate,
                _ => CGEventFlags::empty(),
            };

            let kc = char_to_macos_keycode(key_char.chars().next().unwrap_or('a'));
            (kc, flag)
        }
        _ => return,
    };

    if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), keycode, true) {
        event.set_flags(flags);
        event.post(core_graphics::event::CGEventTapLocation::HID);
    }
    if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), keycode, false) {
        event.set_flags(CGEventFlags::empty());
        event.post(core_graphics::event::CGEventTapLocation::HID);
    }
    std::thread::sleep(std::time::Duration::from_millis(20));
}

fn char_to_macos_keycode(c: char) -> u16 {
    match c {
        'a' => 0x00,
        'b' => 0x0B,
        'c' => 0x08,
        'd' => 0x02,
        'e' => 0x0E,
        'f' => 0x03,
        'g' => 0x05,
        'h' => 0x04,
        'i' => 0x22,
        'j' => 0x26,
        'k' => 0x28,
        'l' => 0x25,
        'm' => 0x2E,
        'n' => 0x2D,
        'o' => 0x1F,
        'p' => 0x23,
        'q' => 0x0C,
        'r' => 0x0F,
        's' => 0x01,
        't' => 0x11,
        'u' => 0x20,
        'v' => 0x09,
        'w' => 0x0D,
        'x' => 0x07,
        'y' => 0x10,
        'z' => 0x06,
        '0' => 0x1D,
        '1' => 0x12,
        '2' => 0x13,
        '3' => 0x14,
        '4' => 0x15,
        '5' => 0x17,
        '6' => 0x16,
        '7' => 0x1A,
        '8' => 0x1C,
        '9' => 0x19,
        _ => 0x00,
    }
}
