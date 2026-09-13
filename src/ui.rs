use crate::backend::CURL_TIMEOUT_OPTIONS;
use crate::config::save_config;
use crate::ffi::geany::{document_get_current, ui_lookup_widget, GeanyKeyGroup, GeanyPlugin};
use crate::ffi::glib::*;
use crate::ffi::gtk::*;
use crate::ffi::scintilla::{scintilla_send_message, ScintillaObject};
use crate::globals::{
    with_global_state, ACTIVE_PRESET_INDEX, CURL_TIMEOUT_INDEX, MAX_TOKENS,
    THINKING_LOG_ENABLED, THINKING_LOG_LOCATION, THINKING_LOG_LOCATION_MESSAGE_WINDOW,
    THINKING_LOG_LOCATION_SIDEBAR, THINKING_LOG_MSGWIN_SPLIT,
};
use crate::request::ask_copilot;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::Ordering;

const MAX_TOKEN_STEPS: &[i32] = &[
    0, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072, 262144,
];
const CUSTOM_MAX_TOKENS_VALUE: i32 = -1;
const SCI_STYLEGETFORE: u32 = 2481;
const SCI_STYLEGETBACK: u32 = 2482;
const SCI_STYLEGETFONT: u32 = 2486;
const STYLE_DEFAULT: usize = 0;

pub struct PluginData {
    pub tool_button: *mut GtkWidget,
    pub statusbar_preset_box: *mut GtkWidget,
    pub statusbar_preset_combo: *mut GtkWidget,
    pub updating_statusbar_preset_combo: bool,
    pub statusbar_timeout_box: *mut GtkWidget,
    pub statusbar_timeout_combo: *mut GtkWidget,
    pub updating_statusbar_timeout_combo: bool,
    pub statusbar_max_tokens_box: *mut GtkWidget,
    pub statusbar_max_tokens_combo: *mut GtkWidget,
    pub max_token_values: Vec<i32>,
    pub updating_statusbar_max_tokens_combo: bool,
    pub thinking_log_panel: *mut GtkWidget,
    pub thinking_view: *mut GtkWidget,
    pub payload_view: *mut GtkWidget,
    pub error_view: *mut GtkWidget,
    pub thinking_log_buffer: *mut GtkTextBuffer,
    pub thinking_log_payload_buffer: *mut GtkTextBuffer,
    pub thinking_log_error_buffer: *mut GtkTextBuffer,
    pub thinking_log_notebook: *mut GtkWidget,
    pub thinking_log_status_label: *mut GtkWidget,
    pub thinking_log_stats_label: *mut GtkWidget,
    pub thinking_log_ask_button: *mut GtkWidget,
    pub thinking_log_stop_button: *mut GtkWidget,
    pub thinking_log_cancel_button: *mut GtkWidget,
    pub thinking_log_paned: *mut GtkWidget,
    pub thinking_log_host_paned: *mut GtkWidget,
    pub thinking_log_editor: *mut GtkWidget,
    pub thinking_log_msgwin_notebook: *mut GtkWidget,
    pub thinking_log_location: i32,
    pub msgwin_paned_last_width: i32,
    pub setting_msgwin_paned_position: bool,
}

pub static mut P_DATA: *mut PluginData = ptr::null_mut();

fn scintilla_color_to_rgba(color: isize) -> GdkRGBA {
    let color = color as u32;
    GdkRGBA {
        red: (color & 0xff) as f64 / 255.0,
        green: ((color >> 8) & 0xff) as f64 / 255.0,
        blue: ((color >> 16) & 0xff) as f64 / 255.0,
        alpha: 1.0,
    }
}

unsafe fn apply_editor_style(text_view: *mut GtkWidget, sci: *mut ScintillaObject) {
    if text_view.is_null() || sci.is_null() {
        return;
    }
    let foreground = scintilla_color_to_rgba(scintilla_send_message(
        sci,
        SCI_STYLEGETFORE,
        STYLE_DEFAULT,
        0,
    ));
    let background = scintilla_color_to_rgba(scintilla_send_message(
        sci,
        SCI_STYLEGETBACK,
        STYLE_DEFAULT,
        0,
    ));
    for state in [GTK_STATE_FLAG_NORMAL, GTK_STATE_FLAG_INSENSITIVE] {
        gtk_widget_override_color(text_view, state, &foreground);
        gtk_widget_override_background_color(text_view, state, &background);
    }

    // SCI_STYLEGETFONT copies the whole font name with no bounds check; ask
    // for the length first (null buffer) instead of trusting a fixed buffer.
    let font_len = scintilla_send_message(sci, SCI_STYLEGETFONT, STYLE_DEFAULT, 0);
    if font_len <= 0 {
        return;
    }
    let mut font = vec![0i8; font_len as usize + 1];
    scintilla_send_message(
        sci,
        SCI_STYLEGETFONT,
        STYLE_DEFAULT,
        font.as_mut_ptr() as isize,
    );
    if font[0] == 0 {
        return;
    }
    let font = CStr::from_ptr(font.as_ptr());
    let description = pango_font_description_new();
    if description.is_null() {
        return;
    }
    pango_font_description_set_family(description, font.as_ptr());
    gtk_widget_override_font(text_view, description);
    pango_font_description_free(description);
}

pub unsafe fn update_statusbar_preset_combo() {
    if P_DATA.is_null() {
        return;
    }

    let pd = &mut *P_DATA;
    if pd.statusbar_preset_combo.is_null() {
        return;
    }

    pd.updating_statusbar_preset_combo = true;

    let active_idx = ACTIVE_PRESET_INDEX.load(Ordering::SeqCst);
    gtk_combo_box_set_active(pd.statusbar_preset_combo as *mut _, active_idx);

    pd.updating_statusbar_preset_combo = false;
}

pub unsafe fn update_statusbar_timeout_combo() {
    if P_DATA.is_null() {
        return;
    }

    let pd = &mut *P_DATA;
    if pd.statusbar_timeout_combo.is_null() {
        return;
    }

    pd.updating_statusbar_timeout_combo = true;

    let timeout_idx = CURL_TIMEOUT_INDEX.load(Ordering::SeqCst);
    gtk_combo_box_set_active(pd.statusbar_timeout_combo as *mut _, timeout_idx);

    pd.updating_statusbar_timeout_combo = false;
}

pub unsafe fn update_statusbar_max_tokens_combo() {
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if pd.statusbar_max_tokens_combo.is_null() {
        return;
    }

    let max_tokens = MAX_TOKENS.load(Ordering::SeqCst).max(0);
    pd.updating_statusbar_max_tokens_combo = true;
    pd.max_token_values = MAX_TOKEN_STEPS.to_vec();
    if max_tokens > 0 && !pd.max_token_values.contains(&max_tokens) {
        pd.max_token_values.push(max_tokens);
    }
    pd.max_token_values.push(CUSTOM_MAX_TOKENS_VALUE);

    gtk_combo_box_text_remove_all(pd.statusbar_max_tokens_combo as *mut _);
    for value in &pd.max_token_values {
        let label = match *value {
            0 => "Default".to_string(),
            CUSTOM_MAX_TOKENS_VALUE => "Custom…".to_string(),
            value if !MAX_TOKEN_STEPS.contains(&value) => format!("Custom: {}", value),
            value => value.to_string(),
        };
        gtk_combo_box_text_append_text(
            pd.statusbar_max_tokens_combo as *mut _,
            CString::new(label).unwrap().as_ptr(),
        );
    }
    let active = pd
        .max_token_values
        .iter()
        .position(|value| *value == max_tokens)
        .unwrap_or(pd.max_token_values.len() - 1);
    gtk_combo_box_set_active(pd.statusbar_max_tokens_combo as *mut _, active as i32);
    pd.updating_statusbar_max_tokens_combo = false;
}

pub unsafe extern "C" fn on_statusbar_preset_changed(
    combo: *mut GtkComboBox,
    user_data: GPointer,
) {
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if pd.updating_statusbar_preset_combo {
        return;
    }

    let selected_idx = gtk_combo_box_get_active(combo);
    if selected_idx >= 0 {
        ACTIVE_PRESET_INDEX.store(selected_idx, Ordering::SeqCst);
        with_global_state(|state| {
            if (selected_idx as usize) < state.presets.len() {
                let p = &state.presets[selected_idx as usize];
                state.backend_type = p.backend_type;
                state.upstream_uri = p.uri.clone();
                state.model_name = p.model.clone();
                state.system_prompt = p.system_prompt.clone();
                state.api_key = p.api_key.clone();
                state.temperature = p.temperature.clone();
                state.include_language_hint = p.include_language_hint;
                state.insert_mode = p.insert_mode;
            }
        });
        save_config(user_data as *mut GeanyPlugin);
    }
}

pub unsafe extern "C" fn on_statusbar_timeout_changed(
    combo: *mut GtkComboBox,
    user_data: GPointer,
) {
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if pd.updating_statusbar_timeout_combo {
        return;
    }

    let selected_idx = gtk_combo_box_get_active(combo);
    if selected_idx >= 0 {
        CURL_TIMEOUT_INDEX.store(selected_idx, Ordering::SeqCst);
        save_config(user_data as *mut GeanyPlugin);
    }
}

pub unsafe extern "C" fn on_statusbar_max_tokens_changed(
    combo: *mut GtkComboBox,
    user_data: GPointer,
) {
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if pd.updating_statusbar_max_tokens_combo {
        return;
    }
    let active = gtk_combo_box_get_active(combo);
    if active < 0 || (active as usize) >= pd.max_token_values.len() {
        return;
    }
    let selected = pd.max_token_values[active as usize];
    if selected != CUSTOM_MAX_TOKENS_VALUE {
        MAX_TOKENS.store(selected, Ordering::SeqCst);
        save_config(user_data as *mut GeanyPlugin);
        return;
    }

    let plugin = user_data as *mut GeanyPlugin;
    let parent = if !plugin.is_null()
        && !(*plugin).geany_data.is_null()
        && !(*(*plugin).geany_data).main_widgets.is_null()
    {
        (*(*plugin).geany_data)
            .main_widgets
            .as_ref()
            .map_or(ptr::null_mut(), |main_widgets| (*main_widgets).window)
    } else {
        ptr::null_mut()
    };
    let dialog = gtk_dialog_new_with_buttons(
        CString::new("Custom Max Tokens").unwrap().as_ptr(),
        parent,
        GTK_DIALOG_MODAL,
        CString::new("_Cancel").unwrap().as_ptr(),
        GTK_RESPONSE_CANCEL,
        CString::new("_Set").unwrap().as_ptr(),
        GTK_RESPONSE_OK,
        ptr::null::<c_char>(),
    );
    let content = gtk_dialog_get_content_area(dialog as *mut _);
    let label = gtk_label_new(
        CString::new("Maximum generated tokens (0 uses the server default)")
            .unwrap()
            .as_ptr(),
    );
    let entry = gtk_entry_new();
    let current = CString::new(MAX_TOKENS.load(Ordering::SeqCst).max(0).to_string()).unwrap();
    gtk_entry_set_text(entry as *mut _, current.as_ptr());
    gtk_box_pack_start(content as *mut _, label, G_FALSE, G_FALSE, 6);
    gtk_box_pack_start(content as *mut _, entry, G_FALSE, G_FALSE, 6);
    gtk_widget_show_all(dialog);
    let accepted = gtk_dialog_run(dialog as *mut _) == GTK_RESPONSE_OK;
    let value = if accepted {
        let text = gtk_entry_get_text(entry as *mut _);
        if text.is_null() {
            0
        } else {
            CStr::from_ptr(text)
                .to_string_lossy()
                .trim()
                .parse::<i32>()
                .unwrap_or(0)
                .max(0)
        }
    } else {
        MAX_TOKENS.load(Ordering::SeqCst).max(0)
    };
    gtk_widget_destroy(dialog);
    MAX_TOKENS.store(value, Ordering::SeqCst);
    update_statusbar_max_tokens_combo();
    save_config(plugin);
}

pub unsafe fn install_statusbar_preset_combo(plugin: *mut GeanyPlugin) {
    if P_DATA.is_null() {
        P_DATA = Box::into_raw(Box::new(PluginData {
            tool_button: ptr::null_mut(),
            statusbar_preset_box: ptr::null_mut(),
            statusbar_preset_combo: ptr::null_mut(),
            updating_statusbar_preset_combo: false,
            statusbar_timeout_box: ptr::null_mut(),
            statusbar_timeout_combo: ptr::null_mut(),
            updating_statusbar_timeout_combo: false,
            statusbar_max_tokens_box: ptr::null_mut(),
            statusbar_max_tokens_combo: ptr::null_mut(),
            max_token_values: Vec::new(),
            updating_statusbar_max_tokens_combo: false,
            thinking_log_panel: ptr::null_mut(),
            thinking_view: ptr::null_mut(),
            payload_view: ptr::null_mut(),
            error_view: ptr::null_mut(),
            thinking_log_buffer: ptr::null_mut(),
            thinking_log_payload_buffer: ptr::null_mut(),
            thinking_log_error_buffer: ptr::null_mut(),
            thinking_log_notebook: ptr::null_mut(),
            thinking_log_status_label: ptr::null_mut(),
            thinking_log_stats_label: ptr::null_mut(),
            thinking_log_ask_button: ptr::null_mut(),
            thinking_log_stop_button: ptr::null_mut(),
            thinking_log_cancel_button: ptr::null_mut(),
            thinking_log_paned: ptr::null_mut(),
            thinking_log_host_paned: ptr::null_mut(),
            thinking_log_editor: ptr::null_mut(),
            thinking_log_msgwin_notebook: ptr::null_mut(),
            thinking_log_location: 0,
            msgwin_paned_last_width: 0,
            setting_msgwin_paned_position: false,
        }));
    }

    let pd = &mut *P_DATA;

    let preset_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
    let preset_combo = gtk_combo_box_text_new();

    with_global_state(|state| {
        for preset in &state.presets {
            let c_label = CString::new(preset.name.as_str()).unwrap();
            gtk_combo_box_text_append_text(preset_combo as *mut _, c_label.as_ptr());
        }
    });

    pd.statusbar_preset_box = preset_box;
    pd.statusbar_preset_combo = preset_combo;
    update_statusbar_preset_combo();

    let c_signal = CString::new("changed").unwrap();
    g_signal_connect_data(
        preset_combo as GPointer,
        c_signal.as_ptr(),
        Some(std::mem::transmute::<
            unsafe extern "C" fn(*mut GtkComboBox, GPointer),
            unsafe extern "C" fn(),
        >(on_statusbar_preset_changed)),
        plugin as GPointer,
        None,
        0,
    );

    let c_label = CString::new("Copilot:").unwrap();
    let label = gtk_label_new(c_label.as_ptr());
    gtk_box_pack_start(preset_box as *mut _, label, G_FALSE, G_FALSE, 0);
    gtk_widget_set_margin_start(preset_box, 6);
    gtk_widget_set_margin_end(preset_box, 6);
    gtk_widget_set_size_request(preset_combo, 135, -1);
    let c_tooltip = CString::new("Copilot backend preset").unwrap();
    gtk_widget_set_tooltip_text(preset_combo, c_tooltip.as_ptr());
    gtk_box_pack_start(preset_box as *mut _, preset_combo, G_FALSE, G_FALSE, 0);
    gtk_widget_show_all(preset_box);

    let timeout_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
    let timeout_combo = gtk_combo_box_text_new();

    for opt in CURL_TIMEOUT_OPTIONS {
        let c_label = CString::new(opt.label).unwrap();
        gtk_combo_box_text_append_text(timeout_combo as *mut _, c_label.as_ptr());
    }

    pd.statusbar_timeout_box = timeout_box;
    pd.statusbar_timeout_combo = timeout_combo;
    update_statusbar_timeout_combo();

    g_signal_connect_data(
        timeout_combo as GPointer,
        c_signal.as_ptr(),
        Some(std::mem::transmute::<
            unsafe extern "C" fn(*mut GtkComboBox, GPointer),
            unsafe extern "C" fn(),
        >(on_statusbar_timeout_changed)),
        plugin as GPointer,
        None,
        0,
    );

    let c_timeout_label = CString::new("Timeout:").unwrap();
    let timeout_label = gtk_label_new(c_timeout_label.as_ptr());
    gtk_box_pack_start(timeout_box as *mut _, timeout_label, G_FALSE, G_FALSE, 0);
    gtk_widget_set_margin_start(timeout_box, 6);
    gtk_widget_set_margin_end(timeout_box, 6);
    gtk_widget_set_size_request(timeout_combo, 80, -1);
    let c_timeout_tooltip = CString::new("Curl request timeout").unwrap();
    gtk_widget_set_tooltip_text(timeout_combo, c_timeout_tooltip.as_ptr());
    gtk_box_pack_start(timeout_box as *mut _, timeout_combo, G_FALSE, G_FALSE, 0);
    gtk_widget_show_all(timeout_box);

    let max_tokens_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
    let max_tokens_combo = gtk_combo_box_text_new();
    pd.statusbar_max_tokens_box = max_tokens_box;
    pd.statusbar_max_tokens_combo = max_tokens_combo;
    update_statusbar_max_tokens_combo();
    g_signal_connect_data(
        max_tokens_combo as GPointer,
        c_signal.as_ptr(),
        Some(std::mem::transmute::<
            unsafe extern "C" fn(*mut GtkComboBox, GPointer),
            unsafe extern "C" fn(),
        >(on_statusbar_max_tokens_changed)),
        plugin as GPointer,
        None,
        0,
    );
    let max_tokens_label = gtk_label_new(CString::new("Tokens:").unwrap().as_ptr());
    gtk_box_pack_start(max_tokens_box as *mut _, max_tokens_label, G_FALSE, G_FALSE, 0);
    gtk_widget_set_margin_start(max_tokens_box, 6);
    gtk_widget_set_margin_end(max_tokens_box, 6);
    gtk_widget_set_size_request(max_tokens_combo, 100, -1);
    gtk_widget_set_tooltip_text(
        max_tokens_combo,
        CString::new("Maximum generated tokens").unwrap().as_ptr(),
    );
    gtk_box_pack_start(max_tokens_box as *mut _, max_tokens_combo, G_FALSE, G_FALSE, 0);
    gtk_widget_show_all(max_tokens_box);

    let main_widgets = (*(*plugin).geany_data).main_widgets;
    let window = (*main_widgets).window;
    if window.is_null() {
        return;
    }

    let c_statusbar = CString::new("statusbar").unwrap();
    let statusbar = ui_lookup_widget(window as *mut _, c_statusbar.as_ptr());
    if statusbar.is_null() {
        return;
    }

    let message_area = gtk_statusbar_get_message_area(statusbar);
    if message_area.is_null() {
        return;
    }

    gtk_box_pack_start(message_area as *mut _, preset_box, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(message_area as *mut _, timeout_box, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(message_area as *mut _, max_tokens_box, G_FALSE, G_FALSE, 0);
}

pub unsafe extern "C" fn on_msgwin_paned_size_allocate(
    widget: *mut GtkWidget,
    allocation: *mut GtkAllocation,
    _user_data: GPointer,
) {
    if widget.is_null() || allocation.is_null() || P_DATA.is_null() {
        return;
    }
    let width = (*allocation).width;
    if width <= 50 {
        return;
    }
    let pd = &mut *P_DATA;
    if pd.msgwin_paned_last_width != width {
        pd.msgwin_paned_last_width = width;
        let split = THINKING_LOG_MSGWIN_SPLIT.load(Ordering::SeqCst).clamp(10, 95);
        let pos = (width * split) / 100;
        pd.setting_msgwin_paned_position = true;
        gtk_paned_set_position(widget as *mut _, pos);
        pd.setting_msgwin_paned_position = false;
    }
}

pub unsafe extern "C" fn on_msgwin_paned_notify_position(
    widget: *mut GtkWidget,
    _pspec: GPointer,
    _user_data: GPointer,
) {
    if widget.is_null() || P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if pd.setting_msgwin_paned_position {
        return;
    }
    let width = pd.msgwin_paned_last_width;
    if width > 100 {
        let pos = gtk_paned_get_position(widget as *mut _);
        if pos > 0 && pos < width {
            let pct = ((pos as f64) / (width as f64) * 100.0).round() as i32;
            let clamped = pct.clamp(10, 95);
            THINKING_LOG_MSGWIN_SPLIT.store(clamped, Ordering::SeqCst);
        }
    }
}

pub unsafe extern "C" fn on_msgwin_paned_button_release(
    _widget: *mut GtkWidget,
    _event: GPointer,
    user_data: GPointer,
) -> GBoolean {
    if !user_data.is_null() {
        save_config(user_data as *mut GeanyPlugin);
    }
    G_FALSE
}

/// Create or remove a dedicated thinking log panel in either the sidebar dock or the message window.
/// When the option is off, the panel and its GtkTextBuffers do not exist,
/// so no UI-side reasoning history is kept.
pub unsafe fn set_thinking_log_enabled(plugin: *mut GeanyPlugin, enabled: bool) {
    if !enabled {
        THINKING_LOG_ENABLED.store(0, Ordering::SeqCst);
    }
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;

    if !enabled {
        remove_thinking_log_panel(pd);
        return;
    }

    let target_location = THINKING_LOG_LOCATION.load(Ordering::SeqCst);
    if !pd.thinking_log_panel.is_null() {
        if pd.thinking_log_location == target_location {
            THINKING_LOG_ENABLED.store(1, Ordering::SeqCst);
            return;
        }
        remove_thinking_log_panel(pd);
    }
    if plugin.is_null() || (*plugin).geany_data.is_null() {
        THINKING_LOG_ENABLED.store(0, Ordering::SeqCst);
        return;
    }
    let main_widgets = (*(*plugin).geany_data).main_widgets;
    if main_widgets.is_null() {
        THINKING_LOG_ENABLED.store(0, Ordering::SeqCst);
        return;
    }

    if target_location == THINKING_LOG_LOCATION_SIDEBAR {
        if (*main_widgets).notebook.is_null() {
            THINKING_LOG_ENABLED.store(0, Ordering::SeqCst);
            return;
        }
        let editor = (*main_widgets).notebook;
        let host_paned = gtk_widget_get_parent(editor);
        // Geany's central layout is hpaned1: its second child is the document
        // notebook.  Only alter that verified layout; never guess at a container.
        if host_paned.is_null() || gtk_paned_get_child2(host_paned as *mut _) != editor {
            THINKING_LOG_ENABLED.store(0, Ordering::SeqCst);
            return;
        }
    } else if target_location == THINKING_LOG_LOCATION_MESSAGE_WINDOW {
        if (*main_widgets).message_window_notebook.is_null() {
            THINKING_LOG_ENABLED.store(0, Ordering::SeqCst);
            return;
        }
    }

    let heading = gtk_label_new(CString::new("Copilot").unwrap().as_ptr());
    let ask_button = gtk_button_new_with_label(CString::new("Ask").unwrap().as_ptr());
    let stop_button = gtk_button_new_with_label(CString::new("Stop").unwrap().as_ptr());
    let cancel_button = gtk_button_new_with_label(CString::new("Cancel").unwrap().as_ptr());
    let clear_button = gtk_button_new_with_label(CString::new("Clear").unwrap().as_ptr());
    let settings_button = gtk_button_new_with_label(CString::new("Settings").unwrap().as_ptr());
    gtk_widget_set_tooltip_text(
        settings_button,
        CString::new("Configure Geany Copilot preferences").unwrap().as_ptr(),
    );
    let status_label = gtk_label_new(CString::new("Ready").unwrap().as_ptr());
    let stats_label = gtk_label_new(CString::new("No active request").unwrap().as_ptr());
    let notebook = gtk_notebook_new();
    let thinking_scrolled = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
    let thinking_view = gtk_text_view_new();
    let payload_scrolled = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
    let payload_view = gtk_text_view_new();
    let error_scrolled = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
    let error_view = gtk_text_view_new();
    gtk_label_set_xalign(heading as *mut _, 0.0);
    gtk_label_set_xalign(status_label as *mut _, 0.0);
    gtk_label_set_xalign(stats_label as *mut _, 0.0);
    gtk_label_set_line_wrap(status_label as *mut _, G_TRUE);
    gtk_label_set_line_wrap(stats_label as *mut _, G_TRUE);
    for view in [thinking_view, payload_view, error_view] {
        gtk_text_view_set_editable(view as *mut _, G_FALSE);
        gtk_text_view_set_cursor_visible(view as *mut _, G_FALSE);
        gtk_text_view_set_wrap_mode(view as *mut _, GTK_WRAP_WORD_CHAR);
        gtk_text_view_set_monospace(view as *mut _, G_TRUE);
    }
    let document = document_get_current();
    if !document.is_null() && !(*document).editor.is_null() {
        let sci = (*(*document).editor).sci;
        apply_editor_style(thinking_view, sci);
        apply_editor_style(payload_view, sci);
        apply_editor_style(error_view, sci);
    }
    gtk_scrolled_window_set_policy(
        thinking_scrolled as *mut _,
        GTK_POLICY_AUTOMATIC,
        GTK_POLICY_AUTOMATIC,
    );
    gtk_scrolled_window_set_policy(
        payload_scrolled as *mut _,
        GTK_POLICY_AUTOMATIC,
        GTK_POLICY_AUTOMATIC,
    );
    gtk_scrolled_window_set_policy(
        error_scrolled as *mut _,
        GTK_POLICY_AUTOMATIC,
        GTK_POLICY_AUTOMATIC,
    );
    if target_location == THINKING_LOG_LOCATION_SIDEBAR {
        gtk_scrolled_window_set_min_content_width(thinking_scrolled as *mut _, 268);
        gtk_scrolled_window_set_min_content_width(payload_scrolled as *mut _, 268);
        gtk_scrolled_window_set_min_content_width(error_scrolled as *mut _, 268);
    }
    gtk_container_add(thinking_scrolled as *mut _, thinking_view);
    gtk_container_add(payload_scrolled as *mut _, payload_view);
    gtk_container_add(error_scrolled as *mut _, error_view);
    gtk_notebook_append_page(
        notebook as *mut _,
        thinking_scrolled,
        gtk_label_new(CString::new("Thinking").unwrap().as_ptr()),
    );
    gtk_notebook_append_page(
        notebook as *mut _,
        payload_scrolled,
        gtk_label_new(CString::new("Payload").unwrap().as_ptr()),
    );
    gtk_notebook_append_page(
        notebook as *mut _,
        error_scrolled,
        gtk_label_new(CString::new("Errors").unwrap().as_ptr()),
    );
    gtk_widget_set_sensitive(stop_button, G_FALSE);
    gtk_widget_set_sensitive(cancel_button, G_FALSE);

    let clicked = CString::new("clicked").unwrap();
    g_signal_connect_data(
        ask_button as GPointer,
        clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_panel_ask_clicked)),
        plugin as GPointer,
        None,
        0,
    );
    g_signal_connect_data(
        stop_button as GPointer,
        clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_panel_stop_clicked)),
        plugin as GPointer,
        None,
        0,
    );
    g_signal_connect_data(
        cancel_button as GPointer,
        clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_panel_cancel_clicked)),
        plugin as GPointer,
        None,
        0,
    );
    g_signal_connect_data(
        clear_button as GPointer,
        clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_panel_clear_clicked)),
        plugin as GPointer,
        None,
        0,
    );
    g_signal_connect_data(
        settings_button as GPointer,
        clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_panel_settings_clicked)),
        plugin as GPointer,
        None,
        0,
    );

    let panel: *mut GtkWidget;
    if target_location == THINKING_LOG_LOCATION_SIDEBAR {
        panel = gtk_box_new(GTK_ORIENTATION_VERTICAL, 5);
        let header = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 5);
        let controls = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 5);
        gtk_widget_set_margin_start(panel, 6);
        gtk_widget_set_margin_end(panel, 6);
        gtk_widget_set_margin_top(panel, 6);
        gtk_widget_set_margin_bottom(panel, 6);
        gtk_widget_set_size_request(panel, 280, -1);
        gtk_scrolled_window_set_min_content_width(thinking_scrolled as *mut _, 268);
        gtk_scrolled_window_set_min_content_width(payload_scrolled as *mut _, 268);
        gtk_scrolled_window_set_min_content_width(error_scrolled as *mut _, 268);

        gtk_box_pack_start(header as *mut _, heading, G_TRUE, G_TRUE, 0);
        gtk_box_pack_start(header as *mut _, clear_button, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(header as *mut _, settings_button, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(controls as *mut _, ask_button, G_TRUE, G_TRUE, 0);
        gtk_box_pack_start(controls as *mut _, stop_button, G_TRUE, G_TRUE, 0);
        gtk_box_pack_start(controls as *mut _, cancel_button, G_TRUE, G_TRUE, 0);
        gtk_box_pack_start(panel as *mut _, header, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(panel as *mut _, controls, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(panel as *mut _, status_label, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(panel as *mut _, notebook, G_TRUE, G_TRUE, 0);
        gtk_box_pack_start(panel as *mut _, stats_label, G_FALSE, G_FALSE, 0);

        let editor = (*main_widgets).notebook;
        let host_paned = gtk_widget_get_parent(editor);
        let right_paned = gtk_paned_new(GTK_ORIENTATION_HORIZONTAL);
        // GtkContainer owns the child, so hold a temporary ref over the move.
        g_object_ref(editor as GPointer);
        gtk_container_remove(host_paned as *mut _, editor);
        gtk_paned_pack1(right_paned as *mut _, editor, G_TRUE, G_TRUE);
        g_object_unref(editor as GPointer);
        gtk_paned_pack2(right_paned as *mut _, panel, G_FALSE, G_FALSE);
        gtk_paned_pack2(host_paned as *mut _, right_paned, G_TRUE, G_TRUE);

        pd.thinking_log_paned = right_paned;
        pd.thinking_log_host_paned = host_paned;
        pd.thinking_log_editor = editor;
        pd.thinking_log_msgwin_notebook = ptr::null_mut();
        pd.msgwin_paned_last_width = 0;
        gtk_widget_show_all(right_paned);
    } else {
        // Column 1 (left, default 90% width): Thinking/Payload/Errors notebook + Clear & Settings buttons
        let col1 = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
        gtk_widget_set_margin_start(col1, 4);
        gtk_widget_set_margin_end(col1, 4);
        gtk_widget_set_margin_top(col1, 4);
        gtk_widget_set_margin_bottom(col1, 4);

        let col1_header = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
        gtk_box_pack_start(col1_header as *mut _, heading, G_TRUE, G_TRUE, 0);
        gtk_box_pack_start(col1_header as *mut _, clear_button, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(col1_header as *mut _, settings_button, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(col1 as *mut _, col1_header, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(col1 as *mut _, notebook, G_TRUE, G_TRUE, 0);

        // Column 2 (right, remaining width): Buttons: Ask, Stop, Cancel (+ status and stats)
        let col2 = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
        gtk_widget_set_margin_start(col2, 6);
        gtk_widget_set_margin_end(col2, 6);
        gtk_widget_set_margin_top(col2, 4);
        gtk_widget_set_margin_bottom(col2, 4);

        gtk_box_pack_start(col2 as *mut _, ask_button, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(col2 as *mut _, stop_button, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(col2 as *mut _, cancel_button, G_FALSE, G_FALSE, 0);
        gtk_box_pack_start(col2 as *mut _, status_label, G_FALSE, G_FALSE, 4);
        gtk_box_pack_start(col2 as *mut _, stats_label, G_FALSE, G_FALSE, 2);

        let paned = gtk_paned_new(GTK_ORIENTATION_HORIZONTAL);
        gtk_paned_pack1(paned as *mut _, col1, G_TRUE, G_FALSE);
        gtk_paned_pack2(paned as *mut _, col2, G_FALSE, G_FALSE);

        let c_size_allocate = CString::new("size-allocate").unwrap();
        g_signal_connect_data(
            paned as GPointer,
            c_size_allocate.as_ptr(),
            Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, *mut GtkAllocation, GPointer), unsafe extern "C" fn()>(on_msgwin_paned_size_allocate)),
            plugin as GPointer,
            None,
            0,
        );

        let c_notify_position = CString::new("notify::position").unwrap();
        g_signal_connect_data(
            paned as GPointer,
            c_notify_position.as_ptr(),
            Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer, GPointer), unsafe extern "C" fn()>(on_msgwin_paned_notify_position)),
            plugin as GPointer,
            None,
            0,
        );

        let c_button_release = CString::new("button-release-event").unwrap();
        g_signal_connect_data(
            paned as GPointer,
            c_button_release.as_ptr(),
            Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer, GPointer) -> GBoolean, unsafe extern "C" fn()>(on_msgwin_paned_button_release)),
            plugin as GPointer,
            None,
            0,
        );

        panel = paned;
        let msgwin = (*main_widgets).message_window_notebook;
        let tab_label = gtk_label_new(CString::new("Copilot").unwrap().as_ptr());
        gtk_notebook_append_page(msgwin as *mut _, panel, tab_label);

        pd.thinking_log_paned = ptr::null_mut();
        pd.thinking_log_host_paned = ptr::null_mut();
        pd.thinking_log_editor = ptr::null_mut();
        pd.thinking_log_msgwin_notebook = msgwin;
        pd.msgwin_paned_last_width = 0;
        gtk_widget_show_all(panel);
    }

    pd.thinking_log_panel = panel;
    pd.thinking_view = thinking_view;
    pd.payload_view = payload_view;
    pd.error_view = error_view;
    pd.thinking_log_buffer = gtk_text_view_get_buffer(thinking_view as *mut _);
    pd.thinking_log_payload_buffer = gtk_text_view_get_buffer(payload_view as *mut _);
    pd.thinking_log_error_buffer = gtk_text_view_get_buffer(error_view as *mut _);
    pd.thinking_log_notebook = notebook;
    pd.thinking_log_status_label = status_label;
    pd.thinking_log_stats_label = stats_label;
    pd.thinking_log_ask_button = ask_button;
    pd.thinking_log_stop_button = stop_button;
    pd.thinking_log_cancel_button = cancel_button;
    pd.thinking_log_location = target_location;

    THINKING_LOG_ENABLED.store(1, Ordering::SeqCst);
    gtk_notebook_set_current_page(notebook as *mut _, 0);
}

unsafe fn remove_thinking_log_panel(pd: &mut PluginData) {
    let panel = pd.thinking_log_panel;
    let right_paned = pd.thinking_log_paned;
    let host_paned = pd.thinking_log_host_paned;
    let editor = pd.thinking_log_editor;
    let msgwin = pd.thinking_log_msgwin_notebook;

    if !msgwin.is_null() && !panel.is_null() {
        gtk_container_remove(msgwin as *mut _, panel);
    }
    if !right_paned.is_null() && !editor.is_null() {
        g_object_ref(editor as GPointer);
        gtk_container_remove(right_paned as *mut _, editor);
        if !host_paned.is_null() {
            g_object_ref(right_paned as GPointer);
            gtk_container_remove(host_paned as *mut _, right_paned);
            gtk_paned_pack2(host_paned as *mut _, editor, G_TRUE, G_TRUE);
            g_object_unref(right_paned as GPointer);
        }
        g_object_unref(editor as GPointer);
    }
    if !right_paned.is_null() && host_paned.is_null() {
        gtk_widget_destroy(right_paned);
    }
    if !panel.is_null() {
        gtk_widget_destroy(panel);
    }
    pd.thinking_log_panel = ptr::null_mut();
    pd.thinking_view = ptr::null_mut();
    pd.payload_view = ptr::null_mut();
    pd.error_view = ptr::null_mut();
    pd.thinking_log_buffer = ptr::null_mut();
    pd.thinking_log_payload_buffer = ptr::null_mut();
    pd.thinking_log_error_buffer = ptr::null_mut();
    pd.thinking_log_notebook = ptr::null_mut();
    pd.thinking_log_status_label = ptr::null_mut();
    pd.thinking_log_stats_label = ptr::null_mut();
    pd.thinking_log_ask_button = ptr::null_mut();
    pd.thinking_log_stop_button = ptr::null_mut();
    pd.thinking_log_cancel_button = ptr::null_mut();
    pd.thinking_log_paned = ptr::null_mut();
    pd.thinking_log_host_paned = ptr::null_mut();
    pd.thinking_log_editor = ptr::null_mut();
    pd.thinking_log_msgwin_notebook = ptr::null_mut();
    pd.thinking_log_location = 0;
    pd.msgwin_paned_last_width = 0;
    pd.setting_msgwin_paned_position = false;
}

unsafe fn append_log_text(view: *mut GtkWidget, buffer: *mut GtkTextBuffer, text: &str) {
    if buffer.is_null() || text.is_empty() {
        return;
    }
    let c_text = CString::new(text).unwrap_or_default();
    let mut end = std::mem::MaybeUninit::<GtkTextIter>::zeroed().assume_init();
    gtk_text_buffer_get_end_iter(buffer, &mut end);
    gtk_text_buffer_insert(buffer, &mut end, c_text.as_ptr(), -1);

    if !view.is_null() {
        gtk_text_buffer_get_end_iter(buffer, &mut end);
        let mark = gtk_text_buffer_create_mark(buffer, ptr::null(), &end, G_FALSE);
        gtk_text_view_scroll_to_mark(view as *mut _, mark, 0.0, G_TRUE, 0.0, 1.0);
        gtk_text_buffer_delete_mark(buffer, mark);
    }
}

pub unsafe fn append_thinking_log(delta: &str) {
    if delta.is_empty() || P_DATA.is_null() || (*P_DATA).thinking_log_buffer.is_null() {
        return;
    }
    append_log_text((*P_DATA).thinking_view, (*P_DATA).thinking_log_buffer, delta);
}

pub unsafe fn append_copilot_error(status: &str, raw_response: &str) {
    if P_DATA.is_null() || (*P_DATA).thinking_log_error_buffer.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    let timestamp = audit_timestamp();
    let raw_response = if raw_response.trim().is_empty() {
        "(no response body)"
    } else {
        raw_response
    };
    append_log_text(
        pd.error_view,
        pd.thinking_log_error_buffer,
        &format!("[{}] {}\n{}\n\n\n", timestamp, status, raw_response),
    );
    if !pd.thinking_log_notebook.is_null() {
        gtk_notebook_set_current_page(pd.thinking_log_notebook as *mut _, 2);
    }
    if pd.thinking_log_location == THINKING_LOG_LOCATION_MESSAGE_WINDOW
        && !pd.thinking_log_msgwin_notebook.is_null()
    {
        let page_num = gtk_notebook_page_num(
            pd.thinking_log_msgwin_notebook as *mut _,
            pd.thinking_log_panel,
        );
        if page_num >= 0 {
            gtk_notebook_set_current_page(
                pd.thinking_log_msgwin_notebook as *mut _,
                page_num,
            );
        }
    }
}

unsafe fn audit_timestamp() -> String {
    let datetime = g_date_time_new_now_local();
    if datetime.is_null() {
        return "unknown time".to_string();
    }
    let format = CString::new("%Y-%m-%d %H:%M:%S").unwrap();
    let formatted = g_date_time_format(datetime, format.as_ptr());
    g_date_time_unref(datetime);
    if formatted.is_null() {
        return "unknown time".to_string();
    }
    let timestamp = CStr::from_ptr(formatted).to_string_lossy().into_owned();
    g_free(formatted as GPointer);
    timestamp
}

pub unsafe fn begin_copilot_request(
    model: &str,
    url: &str,
    payload: &str,
    uses_authorization: bool,
) {
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if !pd.thinking_log_panel.is_null() {
        let model = if model.is_empty() { "server default" } else { model };
        let timestamp = audit_timestamp();
        append_log_text(
            pd.thinking_view,
            pd.thinking_log_buffer,
            &format!("[{}] Thinking — {}\n", timestamp, model),
        );
        append_log_text(
            pd.payload_view,
            pd.thinking_log_payload_buffer,
            &format!(
                "[{}] Request — {}\nPOST {}\n{}{}\n\n\n",
                timestamp,
                model,
                url,
                if uses_authorization {
                    "Authorization: Bearer [redacted]\n"
                } else {
                    ""
                },
                payload,
            ),
        );
        if !pd.thinking_log_notebook.is_null() {
            gtk_notebook_set_current_page(pd.thinking_log_notebook as *mut _, 0);
        }
        if pd.thinking_log_location == THINKING_LOG_LOCATION_MESSAGE_WINDOW
            && !pd.thinking_log_msgwin_notebook.is_null()
        {
            let page_num = gtk_notebook_page_num(
                pd.thinking_log_msgwin_notebook as *mut _,
                pd.thinking_log_panel,
            );
            if page_num >= 0 {
                gtk_notebook_set_current_page(
                    pd.thinking_log_msgwin_notebook as *mut _,
                    page_num,
                );
            }
        }
    }
    set_copilot_panel_status("Waiting for response...", "0 tokens | 0.0 t/s", true);
}

pub unsafe fn update_copilot_panel_stats(estimated_tokens: usize, tokens_per_second: f64) {
    if P_DATA.is_null() || (*P_DATA).thinking_log_stats_label.is_null() {
        return;
    }
    let stats = CString::new(format!("{} tokens | {:.1} t/s", estimated_tokens, tokens_per_second))
        .unwrap();
    gtk_label_set_text((*P_DATA).thinking_log_stats_label as *mut _, stats.as_ptr());
}

pub unsafe fn finish_copilot_request(status: &str) {
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if !pd.thinking_log_panel.is_null() {
        append_log_text(pd.thinking_view, pd.thinking_log_buffer, "\n\n\n");
    }
    set_copilot_panel_status(status, "No active request", false);
}

pub unsafe fn set_copilot_panel_cancelling(stop: bool) {
    let status = if stop {
        "Stopping — keeping partial response..."
    } else {
        "Cancelling Copilot request..."
    };
    set_copilot_panel_status(status, "Request ending...", false);
}

unsafe fn set_copilot_panel_status(status: &str, stats: &str, active: bool) {
    if P_DATA.is_null() {
        return;
    }
    let pd = &mut *P_DATA;
    if !pd.tool_button.is_null() {
        gtk_widget_set_sensitive(pd.tool_button, if active { G_FALSE } else { G_TRUE });
    }
    if !pd.thinking_log_status_label.is_null() {
        let status = CString::new(status).unwrap_or_default();
        gtk_label_set_text(pd.thinking_log_status_label as *mut _, status.as_ptr());
    }
    if !pd.thinking_log_stats_label.is_null() {
        let stats = CString::new(stats).unwrap_or_default();
        gtk_label_set_text(pd.thinking_log_stats_label as *mut _, stats.as_ptr());
    }
    if !pd.thinking_log_ask_button.is_null() {
        gtk_widget_set_sensitive(pd.thinking_log_ask_button, if active { G_FALSE } else { G_TRUE });
    }
    if !pd.thinking_log_stop_button.is_null() {
        gtk_widget_set_sensitive(pd.thinking_log_stop_button, if active { G_TRUE } else { G_FALSE });
    }
    if !pd.thinking_log_cancel_button.is_null() {
        gtk_widget_set_sensitive(pd.thinking_log_cancel_button, if active { G_TRUE } else { G_FALSE });
    }
}

pub unsafe extern "C" fn on_panel_ask_clicked(_button: *mut GtkWidget, user_data: GPointer) {
    ask_copilot(user_data as *mut GeanyPlugin);
}

pub unsafe extern "C" fn on_panel_stop_clicked(_button: *mut GtkWidget, _user_data: GPointer) {
    crate::request::stop_active_request();
}

pub unsafe extern "C" fn on_panel_cancel_clicked(_button: *mut GtkWidget, _user_data: GPointer) {
    crate::request::cancel_active_request();
}

pub unsafe extern "C" fn on_panel_clear_clicked(_button: *mut GtkWidget, _user_data: GPointer) {
    if P_DATA.is_null() {
        return;
    }
    for buffer in [
        (*P_DATA).thinking_log_buffer,
        (*P_DATA).thinking_log_payload_buffer,
        (*P_DATA).thinking_log_error_buffer,
    ] {
        if !buffer.is_null() {
            gtk_text_buffer_set_text(buffer, CString::new("").unwrap().as_ptr(), -1);
        }
    }
}

pub unsafe extern "C" fn on_panel_settings_clicked(_button: *mut GtkWidget, user_data: GPointer) {
    let plugin = user_data as *mut GeanyPlugin;
    if plugin.is_null() {
        return;
    }
    crate::configure::show_settings_dialog(plugin);
}

pub unsafe fn switch_to_next_preset(plugin: *mut GeanyPlugin) {
    with_global_state(|state| {
        if state.presets.is_empty() {
            return;
        }
        let current_idx = ACTIVE_PRESET_INDEX.load(Ordering::SeqCst) as usize;
        let next_idx = (current_idx + 1) % state.presets.len();
        ACTIVE_PRESET_INDEX.store(next_idx as i32, Ordering::SeqCst);

        let p = &state.presets[next_idx];
        state.backend_type = p.backend_type;
        state.upstream_uri = p.uri.clone();
        state.model_name = p.model.clone();
        state.system_prompt = p.system_prompt.clone();
        state.api_key = p.api_key.clone();
        state.temperature = p.temperature.clone();
        state.include_language_hint = p.include_language_hint;
        state.insert_mode = p.insert_mode;
    });

    update_statusbar_preset_combo();
    save_config(plugin);
}

pub unsafe extern "C" fn on_tool_button_clicked(
    _tool_button: *mut GtkWidget,
    user_data: GPointer,
) {
    let plugin = user_data as *mut GeanyPlugin;
    ask_copilot(plugin);
}

pub unsafe extern "C" fn on_keybinding(
    _key_group: *mut GeanyKeyGroup,
    key_id: GUint,
    user_data: GPointer,
) -> GBoolean {
    let plugin = user_data as *mut GeanyPlugin;
    match key_id {
        0 => { ask_copilot(plugin); G_TRUE }
        1 => { switch_to_next_preset(plugin); G_TRUE }
        _ => G_FALSE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::make_default_preset;
    use crate::globals::test_globals_guard;
    use crate::request::{test_request_data, ACTIVE_REQUEST};
    use crate::test_support::{fake_plugin, temp_dir};

    #[test]
    fn scintilla_colors_unpack_as_bgr() {
        let red = scintilla_color_to_rgba(0x0000FF);
        assert_eq!((red.red, red.green, red.blue, red.alpha), (1.0, 0.0, 0.0, 1.0));
        let green = scintilla_color_to_rgba(0x00FF00);
        assert_eq!((green.red, green.green, green.blue), (0.0, 1.0, 0.0));
        let blue = scintilla_color_to_rgba(0xFF0000);
        assert_eq!((blue.red, blue.green, blue.blue), (0.0, 0.0, 1.0));
        let black = scintilla_color_to_rgba(0);
        assert_eq!((black.red, black.green, black.blue, black.alpha), (0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn audit_timestamps_format_local_time() {
        let ts = unsafe { audit_timestamp() };
        assert_eq!(ts.len(), 19, "{}", ts); // YYYY-MM-DD HH:MM:SS
        assert_eq!(ts.chars().filter(|c| *c == ':').count(), 2);
        assert_eq!(ts.chars().filter(|c| *c == '-').count(), 2);
    }

    #[test]
    fn panel_and_statusbar_calls_are_null_safe_without_gtk() {
        let _guard = test_globals_guard();
        unsafe {
            assert!(P_DATA.is_null());
            update_statusbar_preset_combo();
            update_statusbar_timeout_combo();
            update_statusbar_max_tokens_combo();
            on_statusbar_preset_changed(ptr::null_mut(), ptr::null_mut());
            on_statusbar_timeout_changed(ptr::null_mut(), ptr::null_mut());
            on_statusbar_max_tokens_changed(ptr::null_mut(), ptr::null_mut());
            on_panel_stop_clicked(ptr::null_mut(), ptr::null_mut());
            on_panel_cancel_clicked(ptr::null_mut(), ptr::null_mut());
            on_panel_clear_clicked(ptr::null_mut(), ptr::null_mut());
            on_panel_settings_clicked(ptr::null_mut(), ptr::null_mut());
            append_log_text(ptr::null_mut(), ptr::null_mut(), "x");
            append_thinking_log("x");
            append_copilot_error("status", "raw");
            begin_copilot_request("model", "url", "payload", true);
            update_copilot_panel_stats(1, 1.0);
            finish_copilot_request("done");
            set_copilot_panel_cancelling(true);
            set_copilot_panel_cancelling(false);
            on_msgwin_paned_size_allocate(ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            on_msgwin_paned_notify_position(ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            on_msgwin_paned_button_release(ptr::null_mut(), ptr::null_mut(), ptr::null_mut());

            set_thinking_log_enabled(ptr::null_mut(), false);
            assert_eq!(THINKING_LOG_ENABLED.load(Ordering::SeqCst), 0);
            THINKING_LOG_LOCATION.store(THINKING_LOG_LOCATION_MESSAGE_WINDOW, Ordering::SeqCst);
            set_thinking_log_enabled(ptr::null_mut(), true); // P_DATA null: no-op
            THINKING_LOG_LOCATION.store(THINKING_LOG_LOCATION_SIDEBAR, Ordering::SeqCst);
            set_thinking_log_enabled(ptr::null_mut(), true); // P_DATA null: no-op
            THINKING_LOG_ENABLED.store(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn preset_switching_wraps_persists_and_dispatches_from_keybindings() {
        let _guard = test_globals_guard();
        let dir = temp_dir("switch");
        let mut fake = fake_plugin(&dir);
        unsafe {
            // empty preset list: switching is a no-op (but still saves)
            with_global_state(|state| state.presets.clear());
            ACTIVE_PRESET_INDEX.store(0, Ordering::SeqCst);
            switch_to_next_preset(fake.ptr());
            assert_eq!(ACTIVE_PRESET_INDEX.load(Ordering::SeqCst), 0);

            with_global_state(|state| {
                let mut second = make_default_preset();
                second.name = "Second".to_string();
                second.model = "m2".to_string();
                state.presets = vec![make_default_preset(), second];
            });
            switch_to_next_preset(fake.ptr());
            assert_eq!(ACTIVE_PRESET_INDEX.load(Ordering::SeqCst), 1);
            assert_eq!(with_global_state(|s| s.model_name.clone()), "m2");

            // keybinding id 1 is "next preset" and wraps back to 0
            assert_eq!(on_keybinding(ptr::null_mut(), 1, fake.ptr() as GPointer), G_TRUE);
            assert_eq!(ACTIVE_PRESET_INDEX.load(Ordering::SeqCst), 0);
            assert_eq!(on_keybinding(ptr::null_mut(), 99, ptr::null_mut()), G_FALSE);

            // with a request marked active, every ask entry point is a no-op
            let req = Box::into_raw(test_request_data());
            ACTIVE_REQUEST = req;
            assert_eq!(on_keybinding(ptr::null_mut(), 0, ptr::null_mut()), G_TRUE);
            on_tool_button_clicked(ptr::null_mut(), ptr::null_mut());
            on_panel_ask_clicked(ptr::null_mut(), ptr::null_mut());
            let still_active = ACTIVE_REQUEST;
            assert_eq!(still_active, req);
            ACTIVE_REQUEST = ptr::null_mut();
            drop(Box::from_raw(req));

            // restore defaults for the rest of the suite
            with_global_state(|state| {
                state.presets = vec![make_default_preset()];
                state.model_name = String::new();
            });
            ACTIVE_PRESET_INDEX.store(0, Ordering::SeqCst);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
