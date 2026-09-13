use crate::backend::{
    build_model_list_url, parse_model_list_response, BackendPreset, BackendType, InsertMode,
    CURL_TIMEOUT_OPTIONS,
};
use crate::config::save_config;
use crate::ffi::geany::GeanyPlugin;
use crate::ffi::glib::*;
use crate::ffi::gtk::*;
use crate::globals::{
    with_global_state, ACTIVE_PRESET_INDEX, CURL_TIMEOUT_INDEX, MAX_TOKENS,
    THINKING_LOG_ENABLED, THINKING_LOG_LOCATION, THINKING_LOG_LOCATION_MESSAGE_WINDOW,
    THINKING_LOG_LOCATION_SIDEBAR,
};
use crate::ui::{
    set_thinking_log_enabled, update_statusbar_max_tokens_combo, update_statusbar_preset_combo,
    update_statusbar_timeout_combo,
};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::atomic::Ordering;

const MAX_TOKEN_STEPS: &[i32] = &[
    0, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072, 262144,
];
const CUSTOM_MAX_TOKENS_VALUE: i32 = -1;

pub struct ConfigWidgets {
    pub plugin: *mut GeanyPlugin,
    pub configure_dialog: *mut GtkDialog,
    pub preset_combo: *mut GtkWidget,
    pub preset_name_entry: *mut GtkWidget,
    pub ollama_radio: *mut GtkWidget,
    pub openai_compatible_radio: *mut GtkWidget,
    pub uri_entry: *mut GtkWidget,
    pub model_entry: *mut GtkWidget,
    pub api_key_entry: *mut GtkWidget,
    pub temperature_entry: *mut GtkWidget,
    pub language_hint_check: *mut GtkWidget,
    pub insert_mode_combo: *mut GtkWidget,
    pub delete_preset_button: *mut GtkWidget,
    pub timeout_combo: *mut GtkWidget,
    pub max_tokens_combo: *mut GtkWidget,
    pub thinking_log_check: *mut GtkWidget,
    pub thinking_log_sidebar_radio: *mut GtkWidget,
    pub thinking_log_msgwin_radio: *mut GtkWidget,
    pub max_token_values: Vec<i32>,
    pub presets: Vec<BackendPreset>,
    pub active_preset_index: usize,
    pub applying_preset: bool,
}

unsafe fn safe_get_entry_text(entry: *mut GtkWidget) -> String {
    if entry.is_null() {
        return String::new();
    }
    let ptr = gtk_entry_get_text(entry as *mut _);
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().trim().to_string()
    }
}

unsafe fn get_text_buffer_text(buffer: *mut GtkTextBuffer) -> String {
    if buffer.is_null() {
        return String::new();
    }

    let mut start = std::mem::MaybeUninit::<GtkTextIter>::zeroed().assume_init();
    let mut end = std::mem::MaybeUninit::<GtkTextIter>::zeroed().assume_init();
    gtk_text_buffer_get_bounds(buffer, &mut start, &mut end);
    let text = gtk_text_buffer_get_text(buffer, &start, &end, G_TRUE);
    if text.is_null() {
        return String::new();
    }

    let result = CStr::from_ptr(text).to_string_lossy().into_owned();
    g_free(text as GPointer);
    result
}

unsafe fn config_dialog_parent(widgets: &ConfigWidgets) -> *mut GtkWindow {
    if !widgets.configure_dialog.is_null() {
        return widgets.configure_dialog as *mut GtkWindow;
    }
    if !widgets.plugin.is_null()
        && !(*widgets.plugin).geany_data.is_null()
        && !(*(*widgets.plugin).geany_data).main_widgets.is_null()
    {
        (*(*widgets.plugin).geany_data)
            .main_widgets
            .as_ref()
            .map_or(ptr::null_mut(), |main_widgets| (*main_widgets).window)
    } else {
        ptr::null_mut()
    }
}

unsafe fn config_widgets_backend(widgets: &ConfigWidgets) -> BackendType {
    if gtk_toggle_button_get_active(widgets.openai_compatible_radio) != 0 {
        BackendType::OpenAICompatible
    } else {
        BackendType::Ollama
    }
}

unsafe fn refresh_max_tokens_combo(widgets: &mut ConfigWidgets, max_tokens: i32) {
    widgets.applying_preset = true;
    widgets.max_token_values = MAX_TOKEN_STEPS.to_vec();
    if max_tokens > 0 && !widgets.max_token_values.contains(&max_tokens) {
        widgets.max_token_values.push(max_tokens);
    }
    widgets.max_token_values.push(CUSTOM_MAX_TOKENS_VALUE);

    gtk_combo_box_text_remove_all(widgets.max_tokens_combo as *mut _);
    for value in &widgets.max_token_values {
        let label = match *value {
            0 => "Server default".to_string(),
            CUSTOM_MAX_TOKENS_VALUE => "Custom…".to_string(),
            value if !MAX_TOKEN_STEPS.contains(&value) => format!("Custom: {}", value),
            value => value.to_string(),
        };
        gtk_combo_box_text_append_text(
            widgets.max_tokens_combo as *mut _,
            CString::new(label).unwrap().as_ptr(),
        );
    }
    let active = widgets
        .max_token_values
        .iter()
        .position(|value| *value == max_tokens)
        .unwrap_or(widgets.max_token_values.len() - 1);
    gtk_combo_box_set_active(widgets.max_tokens_combo as *mut _, active as c_int);
    widgets.applying_preset = false;
}

pub unsafe fn refresh_preset_combo(widgets: &mut ConfigWidgets, active_index: usize) {
    widgets.applying_preset = true;

    // Clear combo items
    gtk_combo_box_text_remove_all(widgets.preset_combo as *mut _);

    for preset in &widgets.presets {
        let c_label = CString::new(preset.name.as_str()).unwrap();
        gtk_combo_box_text_append_text(widgets.preset_combo as *mut _, c_label.as_ptr());
    }

    if active_index < widgets.presets.len() {
        gtk_combo_box_set_active(widgets.preset_combo as *mut _, active_index as c_int);
        widgets.active_preset_index = active_index;
    }

    gtk_widget_set_sensitive(widgets.delete_preset_button, if widgets.presets.len() > 1 { 1 } else { 0 });

    widgets.applying_preset = false;
}

pub unsafe fn apply_preset_to_config_widgets(widgets: &mut ConfigWidgets, index: usize) {
    if index >= widgets.presets.len() {
        return;
    }

    widgets.applying_preset = true;
    widgets.active_preset_index = index;
    let preset = &widgets.presets[index];

    gtk_entry_set_text(widgets.preset_name_entry as *mut _, CString::new(preset.name.as_str()).unwrap().as_ptr());
    gtk_entry_set_text(widgets.uri_entry as *mut _, CString::new(preset.uri.as_str()).unwrap().as_ptr());
    gtk_entry_set_placeholder_text(widgets.uri_entry as *mut _, CString::new(preset.backend_type.default_uri()).unwrap().as_ptr());
    gtk_entry_set_text(widgets.model_entry as *mut _, CString::new(preset.model.as_str()).unwrap().as_ptr());
    gtk_entry_set_text(widgets.api_key_entry as *mut _, CString::new(preset.api_key.as_str()).unwrap().as_ptr());
    gtk_entry_set_text(widgets.temperature_entry as *mut _, CString::new(preset.temperature.as_str()).unwrap().as_ptr());
    gtk_toggle_button_set_active(
        widgets.language_hint_check,
        if preset.include_language_hint { G_TRUE } else { G_FALSE },
    );
    let insert_index = match preset.insert_mode {
        InsertMode::Cursor => 0,
        InsertMode::ReplaceSelection => 1,
        InsertMode::AppendAfterSelection => 2,
    };
    gtk_combo_box_set_active(widgets.insert_mode_combo as *mut _, insert_index);

    if preset.backend_type == BackendType::OpenAICompatible {
        gtk_toggle_button_set_active(widgets.openai_compatible_radio as *mut _, G_TRUE);
    } else {
        gtk_toggle_button_set_active(widgets.ollama_radio as *mut _, G_TRUE);
    }

    widgets.applying_preset = false;
}

pub unsafe fn sync_active_preset_from_widgets(widgets: &mut ConfigWidgets) {
    if widgets.active_preset_index >= widgets.presets.len() {
        return;
    }

    let name = safe_get_entry_text(widgets.preset_name_entry);
    let uri = safe_get_entry_text(widgets.uri_entry);
    let model = safe_get_entry_text(widgets.model_entry);
    let api_key = safe_get_entry_text(widgets.api_key_entry);
    let temperature = safe_get_entry_text(widgets.temperature_entry);
    let backend_type = config_widgets_backend(widgets);
    let include_language_hint = gtk_toggle_button_get_active(widgets.language_hint_check) != 0;
    let insert_mode = match gtk_combo_box_get_active(widgets.insert_mode_combo as *mut _) {
        1 => InsertMode::ReplaceSelection,
        2 => InsertMode::AppendAfterSelection,
        _ => InsertMode::Cursor,
    };

    let preset = &mut widgets.presets[widgets.active_preset_index];
    preset.name = if name.is_empty() { format!("Preset {}", widgets.active_preset_index + 1) } else { name };
    preset.backend_type = backend_type;
    preset.uri = uri;
    preset.model = model;
    preset.api_key = api_key;
    preset.temperature = temperature;
    preset.include_language_hint = include_language_hint;
    preset.insert_mode = insert_mode;
}

pub unsafe fn commit_config_widgets(widgets: &mut ConfigWidgets) {
    sync_active_preset_from_widgets(widgets);

    let timeout_idx = gtk_combo_box_get_active(widgets.timeout_combo as *mut _);
    if timeout_idx >= 0 {
        CURL_TIMEOUT_INDEX.store(timeout_idx, Ordering::SeqCst);
    }

    ACTIVE_PRESET_INDEX.store(widgets.active_preset_index as i32, Ordering::SeqCst);
    let thinking_log_enabled = gtk_toggle_button_get_active(widgets.thinking_log_check) != 0;
    THINKING_LOG_ENABLED.store(thinking_log_enabled as i32, Ordering::SeqCst);
    let thinking_log_location = if !widgets.thinking_log_msgwin_radio.is_null()
        && gtk_toggle_button_get_active(widgets.thinking_log_msgwin_radio) != 0
    {
        THINKING_LOG_LOCATION_MESSAGE_WINDOW
    } else {
        THINKING_LOG_LOCATION_SIDEBAR
    };
    THINKING_LOG_LOCATION.store(thinking_log_location, Ordering::SeqCst);

    with_global_state(|state| {
        state.presets = widgets.presets.clone();
        if let Some(active_p) = state.presets.get(widgets.active_preset_index) {
            state.backend_type = active_p.backend_type;
            state.upstream_uri = active_p.uri.clone();
            state.model_name = active_p.model.clone();
            state.system_prompt = active_p.system_prompt.clone();
            state.api_key = active_p.api_key.clone();
            state.temperature = active_p.temperature.clone();
            state.include_language_hint = active_p.include_language_hint;
            state.insert_mode = active_p.insert_mode;
        }
    });

    save_config(widgets.plugin);
    update_statusbar_preset_combo();
    update_statusbar_timeout_combo();
    update_statusbar_max_tokens_combo();
    // Do this last: disabling or moving the panel can destroy this settings form.
    set_thinking_log_enabled(widgets.plugin, thinking_log_enabled);
}

pub unsafe extern "C" fn on_max_tokens_changed(combo: *mut GtkComboBox, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);
    if widgets.applying_preset {
        return;
    }
    let active = gtk_combo_box_get_active(combo);
    if active < 0 || (active as usize) >= widgets.max_token_values.len() {
        return;
    }
    let selected = widgets.max_token_values[active as usize];
    if selected != CUSTOM_MAX_TOKENS_VALUE {
        MAX_TOKENS.store(selected, Ordering::SeqCst);
        return;
    }

    let dialog = gtk_dialog_new_with_buttons(
        CString::new("Custom Max Tokens").unwrap().as_ptr(),
        config_dialog_parent(widgets),
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
        safe_get_entry_text(entry).parse::<i32>().unwrap_or(0).max(0)
    } else {
        MAX_TOKENS.load(Ordering::SeqCst).max(0)
    };
    gtk_widget_destroy(dialog);
    MAX_TOKENS.store(value, Ordering::SeqCst);
    refresh_max_tokens_combo(widgets, value);
}

pub unsafe extern "C" fn on_preset_changed(combo: *mut GtkComboBox, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);
    if widgets.applying_preset {
        return;
    }

    let selected = gtk_combo_box_get_active(combo);
    if selected >= 0 && (selected as usize) < widgets.presets.len() {
        sync_active_preset_from_widgets(widgets);
        apply_preset_to_config_widgets(widgets, selected as usize);
    }
}

pub unsafe extern "C" fn on_add_preset_clicked(_button: *mut GtkWidget, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);

    let new_name = format!("Preset {}", widgets.presets.len() + 1);
    let parent = if !widgets.plugin.is_null()
        && !(*widgets.plugin).geany_data.is_null()
        && !(*(*widgets.plugin).geany_data).main_widgets.is_null()
    {
        (*(*widgets.plugin).geany_data).main_widgets.as_ref().map_or(ptr::null_mut(), |w| (*w).window)
    } else {
        ptr::null_mut()
    };
    let prompt = gtk_dialog_new_with_buttons(
        CString::new("Add Preset").unwrap().as_ptr(),
        parent,
        GTK_DIALOG_MODAL,
        CString::new("_Cancel").unwrap().as_ptr(),
        GTK_RESPONSE_CANCEL,
        CString::new("_Add").unwrap().as_ptr(),
        GTK_RESPONSE_OK,
        ptr::null::<c_char>(),
    );
    let prompt_area = gtk_dialog_get_content_area(prompt as *mut _);
    let prompt_label = gtk_label_new(CString::new("Preset name").unwrap().as_ptr());
    let prompt_entry = gtk_entry_new();
    gtk_entry_set_text(prompt_entry as *mut _, CString::new(new_name.as_str()).unwrap().as_ptr());
    gtk_box_pack_start(prompt_area as *mut _, prompt_label, G_FALSE, G_FALSE, 6);
    gtk_box_pack_start(prompt_area as *mut _, prompt_entry, G_FALSE, G_FALSE, 6);
    gtk_widget_show_all(prompt);
    let accepted = gtk_dialog_run(prompt as *mut _) == GTK_RESPONSE_OK;
    let prompted_name = if accepted { safe_get_entry_text(prompt_entry) } else { String::new() };
    gtk_widget_destroy(prompt);
    if !accepted { return; }
    let new_name = if prompted_name.is_empty() { new_name } else { prompted_name };
    let new_preset = BackendPreset {
        name: new_name,
        backend_type: BackendType::Ollama,
        uri: String::new(),
        model: String::new(),
        system_prompt: String::new(),
        api_key: String::new(),
        temperature: String::new(),
        include_language_hint: true,
        insert_mode: InsertMode::Cursor,
    };

    widgets.presets.push(new_preset);
    let new_index = widgets.presets.len() - 1;
    refresh_preset_combo(widgets, new_index);
    apply_preset_to_config_widgets(widgets, new_index);
}

pub unsafe extern "C" fn on_save_preset_clicked(_button: *mut GtkWidget, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);
    // Refresh labels before commit: turning the dock off can destroy this
    // sidebar settings page and free `widgets` during commit.
    sync_active_preset_from_widgets(widgets);
    refresh_preset_combo(widgets, widgets.active_preset_index);
    commit_config_widgets(widgets);
}

pub unsafe extern "C" fn on_system_prompt_clicked(_button: *mut GtkWidget, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);
    sync_active_preset_from_widgets(widgets);
    if widgets.active_preset_index >= widgets.presets.len() {
        return;
    }

    let dialog = gtk_dialog_new_with_buttons(
        CString::new("System Prompt").unwrap().as_ptr(),
        config_dialog_parent(widgets),
        GTK_DIALOG_MODAL,
        CString::new("_Cancel").unwrap().as_ptr(),
        GTK_RESPONSE_CANCEL,
        CString::new("_Save").unwrap().as_ptr(),
        GTK_RESPONSE_OK,
        ptr::null::<c_char>(),
    );
    gtk_window_set_default_size(dialog as *mut _, 720, 480);
    gtk_window_set_destroy_with_parent(dialog as *mut _, G_TRUE);

    let content_area = gtk_dialog_get_content_area(dialog as *mut _);
    let hint = gtk_label_new(
        CString::new("Sent before the editor context. Leave empty to send no system prompt.")
            .unwrap()
            .as_ptr(),
    );
    gtk_label_set_xalign(hint as *mut _, 0.0);
    gtk_label_set_line_wrap(hint as *mut _, G_TRUE);

    let scrolled = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
    let text_view = gtk_text_view_new();
    gtk_text_view_set_wrap_mode(text_view as *mut _, GTK_WRAP_WORD_CHAR);
    gtk_scrolled_window_set_policy(
        scrolled as *mut _,
        GTK_POLICY_AUTOMATIC,
        GTK_POLICY_AUTOMATIC,
    );
    gtk_scrolled_window_set_min_content_width(scrolled as *mut _, 640);
    gtk_scrolled_window_set_min_content_height(scrolled as *mut _, 320);
    gtk_widget_set_hexpand(scrolled, G_TRUE);
    gtk_widget_set_vexpand(scrolled, G_TRUE);
    gtk_container_add(scrolled as *mut _, text_view);

    let buffer = gtk_text_view_get_buffer(text_view as *mut _);
    let current_prompt = &widgets.presets[widgets.active_preset_index].system_prompt;
    let prompt = CString::new(current_prompt.as_str()).unwrap_or_default();
    gtk_text_buffer_set_text(buffer, prompt.as_ptr(), -1);

    gtk_box_pack_start(content_area as *mut _, hint, G_FALSE, G_FALSE, 8);
    gtk_box_pack_start(content_area as *mut _, scrolled, G_TRUE, G_TRUE, 8);
    gtk_widget_show_all(dialog);

    if gtk_dialog_run(dialog as *mut _) == GTK_RESPONSE_OK {
        let prompt = get_text_buffer_text(buffer);
        widgets.presets[widgets.active_preset_index].system_prompt = if prompt.trim().is_empty() {
            String::new()
        } else {
            prompt
        };
    }

    gtk_widget_destroy(dialog);
}

pub unsafe extern "C" fn on_delete_preset_clicked(_button: *mut GtkWidget, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);
    if widgets.presets.len() <= 1 {
        return;
    }

    let index = widgets.active_preset_index;
    if index < widgets.presets.len() {
        widgets.presets.remove(index);
        let next_index = index.min(widgets.presets.len() - 1);
        refresh_preset_combo(widgets, next_index);
        apply_preset_to_config_widgets(widgets, next_index);
        commit_config_widgets(widgets);
    }
}

pub unsafe extern "C" fn clear_model_on_upstream_change(_widget: *mut GtkWidget, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);
    if widgets.applying_preset {
        return;
    }
    gtk_entry_set_text(widgets.model_entry as *mut _, CString::new("").unwrap().as_ptr());
}

pub unsafe extern "C" fn on_select_model_clicked(_button: *mut GtkWidget, user_data: GPointer) {
    let widgets = &mut *(user_data as *mut ConfigWidgets);

    let uri_str = safe_get_entry_text(widgets.uri_entry);
    let backend = config_widgets_backend(widgets);
    let final_uri = if uri_str.is_empty() { backend.default_uri().to_string() } else { uri_str };

    let url = build_model_list_url(backend, &final_uri);
    let api_key = safe_get_entry_text(widgets.api_key_entry);

    let mut easy = curl::easy::Easy::new();
    let mut error_msg = String::new();
    if let Err(e) = easy
        .url(&url)
        .and_then(|_| easy.timeout(std::time::Duration::from_secs(10)))
    {
        error_msg = format!("curl setup error: {}", e);
    }
    if error_msg.is_empty() && !api_key.is_empty() {
        let mut headers = curl::easy::List::new();
        if headers
            .append(&format!("Authorization: Bearer {}", api_key))
            .is_err()
        {
            error_msg = "curl setup error: invalid authorization header".to_string();
        } else if let Err(e) = easy.http_headers(headers) {
            error_msg = format!("curl setup error: {}", e);
        }
    }

    let mut body = Vec::new();
    if error_msg.is_empty() {
        let res = {
            let mut transfer = easy.transfer();
            match transfer.write_function(|data| {
                body.extend_from_slice(data);
                Ok(data.len())
            }) {
                Ok(()) => transfer.perform(),
                Err(e) => Err(e),
            }
        };
        if let Err(e) = res {
            error_msg = format!("Connection error: {}", e);
        } else if let Ok(status) = easy.response_code() {
            if status >= 400 {
                error_msg = format!("HTTP error {}", status);
            }
        }
    }

    let response_str = String::from_utf8_lossy(&body);

    let models_result = if error_msg.is_empty() {
        parse_model_list_response(&response_str, backend)
    } else {
        Err(error_msg)
    };

    match models_result {
        Ok(models) => {
            let dialog = gtk_dialog_new_with_buttons(
                CString::new("Select Model").unwrap().as_ptr(),
                if !widgets.plugin.is_null() && !(*widgets.plugin).geany_data.is_null() && !(*(*widgets.plugin).geany_data).main_widgets.is_null() { (*(*widgets.plugin).geany_data).main_widgets.as_ref().map_or(ptr::null_mut(), |w| (*w).window) } else { ptr::null_mut() },
                GTK_DIALOG_MODAL,
                CString::new("_Select").unwrap().as_ptr(),
                GTK_RESPONSE_OK,
                CString::new("_Cancel").unwrap().as_ptr(),
                GTK_RESPONSE_CANCEL,
                ptr::null_mut::<c_char>(),
            );

            let content_area = gtk_dialog_get_content_area(dialog as *mut _);
            let combo = gtk_combo_box_text_new();

            for m in &models {
                let c_m = CString::new(m.as_str()).unwrap_or_default();
                gtk_combo_box_text_append_text(combo as *mut _, c_m.as_ptr());
            }

            if !models.is_empty() {
                gtk_combo_box_set_active(combo as *mut _, 0);
            }

            gtk_box_pack_start(content_area as *mut _, combo, G_FALSE, G_FALSE, 8);
            gtk_widget_show_all(dialog);

            if gtk_dialog_run(dialog as *mut _) == GTK_RESPONSE_OK {
                let active = gtk_combo_box_get_active(combo as *mut _);
                if active >= 0 && (active as usize) < models.len() {
                    let selected = &models[active as usize];
                    let c_selected = CString::new(selected.as_str()).unwrap_or_default();
                    gtk_entry_set_text(widgets.model_entry as *mut _, c_selected.as_ptr());
                }
            }

            gtk_widget_destroy(dialog);
        }
        Err(err) => {
            let details = format!(
                "Failed to list models.\n\nURL: {}\nError: {}\n\nResponse:\n{}",
                url, err, if response_str.is_empty() { "(empty)" } else { &response_str }
            );
            // The raw response body can contain NUL bytes; never panic here.
            let c_details = CString::new(details.replace('\0', "")).unwrap_or_default();
            let format = CString::new("%s").unwrap();
            let dialog = gtk_message_dialog_new(
                if !widgets.plugin.is_null() && !(*widgets.plugin).geany_data.is_null() && !(*(*widgets.plugin).geany_data).main_widgets.is_null() { (*(*widgets.plugin).geany_data).main_widgets.as_ref().map_or(ptr::null_mut(), |w| (*w).window) } else { ptr::null_mut() },
                GTK_DIALOG_MODAL,
                GTK_MESSAGE_ERROR,
                GTK_BUTTONS_CLOSE,
                format.as_ptr(),
                c_details.as_ptr(),
            );
            gtk_dialog_run(dialog as *mut _);
            gtk_widget_destroy(dialog);
        }
    }
}

pub unsafe extern "C" fn on_configure_response(
    _dialog: *mut GtkDialog,
    response_id: c_int,
    user_data: GPointer,
) {
    if response_id == GTK_RESPONSE_OK || response_id == GTK_RESPONSE_APPLY {
        let widgets = &mut *(user_data as *mut ConfigWidgets);
        commit_config_widgets(widgets);
    }
}

pub unsafe extern "C" fn free_config_widgets(data: GPointer) {
    if !data.is_null() {
        let _ = Box::from_raw(data as *mut ConfigWidgets);
    }
}

pub unsafe extern "C" fn copilot_plugin_configure(
    plugin: *mut GeanyPlugin,
    dialog: *mut GtkDialog,
    _user_data: GPointer,
) -> *mut GtkWidget {
    crate::config::load_config(plugin);
    set_thinking_log_enabled(
        plugin,
        THINKING_LOG_ENABLED.load(Ordering::SeqCst) != 0,
    );

    build_settings_form(plugin, dialog)
}

/// Shows the Geany Copilot preferences dialog as a modal window.
pub unsafe fn show_settings_dialog(plugin: *mut GeanyPlugin) {
    if plugin.is_null() {
        return;
    }
    crate::config::load_config(plugin);

    let parent = if !(*plugin).geany_data.is_null()
        && !(*(*plugin).geany_data).main_widgets.is_null()
    {
        (*(*plugin).geany_data)
            .main_widgets
            .as_ref()
            .map_or(ptr::null_mut(), |main_widgets| (*main_widgets).window)
    } else {
        ptr::null_mut()
    };

    let title = CString::new("Geany Copilot Preferences").unwrap();
    let cancel_lbl = CString::new("_Cancel").unwrap();
    let apply_lbl = CString::new("_Apply").unwrap();
    let ok_lbl = CString::new("_OK").unwrap();

    let dialog = gtk_dialog_new_with_buttons(
        title.as_ptr(),
        parent,
        GTK_DIALOG_MODAL,
        cancel_lbl.as_ptr(),
        GTK_RESPONSE_CANCEL,
        apply_lbl.as_ptr(),
        GTK_RESPONSE_APPLY,
        ok_lbl.as_ptr(),
        GTK_RESPONSE_OK,
        ptr::null::<c_char>(),
    );
    if dialog.is_null() {
        return;
    }

    gtk_window_set_default_size(dialog as *mut GtkWindow, 520, 580);

    let form = build_settings_form(plugin, dialog as *mut GtkDialog);
    let scrolled = gtk_scrolled_window_new(ptr::null_mut(), ptr::null_mut());
    gtk_scrolled_window_set_policy(
        scrolled as *mut _,
        GTK_POLICY_AUTOMATIC,
        GTK_POLICY_AUTOMATIC,
    );
    gtk_container_add(scrolled as *mut _, form);

    let content_area = gtk_dialog_get_content_area(dialog as *mut _);
    gtk_box_pack_start(content_area as *mut _, scrolled, G_TRUE, G_TRUE, 0);

    gtk_widget_show_all(dialog);

    // Keep the dialog open when the user clicks Apply (the on_configure_response
    // signal handler commits settings on both Apply and OK).  Close on OK, Cancel,
    // or window-close.
    while gtk_dialog_run(dialog as *mut _) == GTK_RESPONSE_APPLY {}

    gtk_widget_destroy(dialog);
}

unsafe extern "C" fn on_settings_form_destroy(_widget: *mut GtkWidget, _data: GPointer) {}

unsafe fn update_thinking_log_controls_sensitivity(widgets: &ConfigWidgets) {
    let active = !widgets.thinking_log_check.is_null()
        && gtk_toggle_button_get_active(widgets.thinking_log_check) != 0;
    if !widgets.thinking_log_sidebar_radio.is_null() {
        gtk_widget_set_sensitive(
            widgets.thinking_log_sidebar_radio,
            if active { G_TRUE } else { G_FALSE },
        );
    }
    if !widgets.thinking_log_msgwin_radio.is_null() {
        gtk_widget_set_sensitive(
            widgets.thinking_log_msgwin_radio,
            if active { G_TRUE } else { G_FALSE },
        );
    }
}

pub unsafe extern "C" fn on_thinking_log_check_toggled(
    _check: *mut GtkWidget,
    user_data: GPointer,
) {
    if user_data.is_null() {
        return;
    }
    let widgets = &*(user_data as *mut ConfigWidgets);
    update_thinking_log_controls_sensitivity(widgets);
}

unsafe fn build_settings_form(
    plugin: *mut GeanyPlugin,
    dialog: *mut GtkDialog,
) -> *mut GtkWidget {

    let box_widget = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
    gtk_widget_set_margin_start(box_widget, 8);
    gtk_widget_set_margin_end(box_widget, 8);
    gtk_widget_set_margin_top(box_widget, 8);
    gtk_widget_set_margin_bottom(box_widget, 8);

    // Preset selector row
    let preset_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    let preset_combo = gtk_combo_box_text_new();
    let add_btn = gtk_button_new_with_label(CString::new("Add").unwrap().as_ptr());
    let save_btn = gtk_button_new_with_label(CString::new("Save").unwrap().as_ptr());
    let delete_btn = gtk_button_new_with_label(CString::new("Delete").unwrap().as_ptr());

    gtk_box_pack_start(preset_box as *mut _, preset_combo, G_TRUE, G_TRUE, 0);
    gtk_box_pack_start(preset_box as *mut _, add_btn, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(preset_box as *mut _, save_btn, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(preset_box as *mut _, delete_btn, G_FALSE, G_FALSE, 0);

    // Form fields
    let name_label = gtk_label_new(CString::new("Preset Name").unwrap().as_ptr());
    let name_entry = gtk_entry_new();

    let backend_label = gtk_label_new(CString::new("Backend").unwrap().as_ptr());
    let ollama_radio = gtk_radio_button_new_with_label_from_widget(
        ptr::null_mut(),
        CString::new("Ollama").unwrap().as_ptr(),
    );
    let openai_radio = gtk_radio_button_new_with_label_from_widget(
        ollama_radio as *mut _,
        CString::new("OpenAI-compatible").unwrap().as_ptr(),
    );
    let radio_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    gtk_box_pack_start(radio_box as *mut _, ollama_radio, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(radio_box as *mut _, openai_radio, G_FALSE, G_FALSE, 0);

    let uri_label = gtk_label_new(CString::new("Upstream URI").unwrap().as_ptr());
    let uri_entry = gtk_entry_new();
    let uri_hint = gtk_label_new(CString::new("Examples: http://localhost:11434, http://localhost:11434/v1").unwrap().as_ptr());

    let model_label = gtk_label_new(CString::new("Model").unwrap().as_ptr());
    let model_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    let model_entry = gtk_entry_new();
    let select_model_btn = gtk_button_new_with_label(CString::new("Select...").unwrap().as_ptr());
    gtk_box_pack_start(model_box as *mut _, model_entry, G_TRUE, G_TRUE, 0);
    gtk_box_pack_start(model_box as *mut _, select_model_btn, G_FALSE, G_FALSE, 0);

    let api_key_label = gtk_label_new(CString::new("API Key").unwrap().as_ptr());
    let api_key_entry = gtk_entry_new();
    gtk_entry_set_visibility(api_key_entry as *mut _, G_FALSE);
    gtk_entry_set_placeholder_text(
        api_key_entry as *mut _,
        CString::new("Optional bearer token").unwrap().as_ptr(),
    );

    let generation_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    let temperature_label = gtk_label_new(CString::new("Temperature").unwrap().as_ptr());
    let temperature_entry = gtk_entry_new();
    gtk_widget_set_size_request(temperature_entry, 78, -1);
    gtk_entry_set_placeholder_text(
        temperature_entry as *mut _,
        CString::new("server default").unwrap().as_ptr(),
    );
    gtk_widget_set_tooltip_text(
        temperature_entry,
        CString::new("0.0 to 2.0; leave blank for the server default").unwrap().as_ptr(),
    );
    let language_hint_check = gtk_check_button_new_with_label(
        CString::new("Language hint").unwrap().as_ptr(),
    );
    gtk_widget_set_tooltip_text(
        language_hint_check,
        CString::new("Adds the current document's programming language to the request when it can be identified from the file extension.")
            .unwrap()
            .as_ptr(),
    );
    gtk_box_pack_start(generation_box as *mut _, temperature_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(generation_box as *mut _, temperature_entry, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(generation_box as *mut _, language_hint_check, G_FALSE, G_FALSE, 6);

    let insert_mode_label = gtk_label_new(CString::new("Response placement").unwrap().as_ptr());
    let insert_mode_combo = gtk_combo_box_text_new();
    for label in ["At cursor", "Replace selection", "After selection (new line)"] {
        gtk_combo_box_text_append_text(insert_mode_combo as *mut _, CString::new(label).unwrap().as_ptr());
    }

    let system_prompt_btn =
        gtk_button_new_with_label(CString::new("System Prompt").unwrap().as_ptr());
    gtk_widget_set_tooltip_text(
        system_prompt_btn,
        CString::new("Edit the system prompt for this preset")
            .unwrap()
            .as_ptr(),
    );

    let thinking_log_check = gtk_check_button_new_with_label(
        CString::new("Show thinking log").unwrap().as_ptr(),
    );
    let thinking_log_enabled = THINKING_LOG_ENABLED.load(Ordering::SeqCst) != 0;
    gtk_toggle_button_set_active(
        thinking_log_check,
        if thinking_log_enabled {
            G_TRUE
        } else {
            G_FALSE
        },
    );
    gtk_widget_set_tooltip_text(
        thinking_log_check,
        CString::new("Capture streamed reasoning and request diagnostics")
            .unwrap()
            .as_ptr(),
    );

    let thinking_log_sidebar_radio = gtk_radio_button_new_with_label_from_widget(
        ptr::null_mut(),
        CString::new("Sidebar").unwrap().as_ptr(),
    );
    let thinking_log_msgwin_radio = gtk_radio_button_new_with_label_from_widget(
        thinking_log_sidebar_radio as *mut _,
        CString::new("Message window").unwrap().as_ptr(),
    );

    let thinking_log_location = THINKING_LOG_LOCATION.load(Ordering::SeqCst);
    if thinking_log_location == THINKING_LOG_LOCATION_MESSAGE_WINDOW {
        gtk_toggle_button_set_active(thinking_log_msgwin_radio as *mut _, G_TRUE);
    } else {
        gtk_toggle_button_set_active(thinking_log_sidebar_radio as *mut _, G_TRUE);
    }

    gtk_widget_set_sensitive(
        thinking_log_sidebar_radio,
        if thinking_log_enabled { G_TRUE } else { G_FALSE },
    );
    gtk_widget_set_sensitive(
        thinking_log_msgwin_radio,
        if thinking_log_enabled { G_TRUE } else { G_FALSE },
    );
    gtk_widget_set_tooltip_text(
        thinking_log_sidebar_radio,
        CString::new("Show thinking log in a dedicated right-side sidebar").unwrap().as_ptr(),
    );
    gtk_widget_set_tooltip_text(
        thinking_log_msgwin_radio,
        CString::new("Show thinking log in a bottom message window tab").unwrap().as_ptr(),
    );

    let thinking_log_loc_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 12);
    gtk_widget_set_margin_start(thinking_log_loc_box, 20);
    gtk_box_pack_start(
        thinking_log_loc_box as *mut _,
        thinking_log_sidebar_radio,
        G_FALSE,
        G_FALSE,
        0,
    );
    gtk_box_pack_start(
        thinking_log_loc_box as *mut _,
        thinking_log_msgwin_radio,
        G_FALSE,
        G_FALSE,
        0,
    );

    let limits_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 8);
    let timeout_label = gtk_label_new(CString::new("Timeout").unwrap().as_ptr());
    let timeout_combo = gtk_combo_box_text_new();
    for opt in CURL_TIMEOUT_OPTIONS {
        let c_lbl = CString::new(opt.label).unwrap();
        gtk_combo_box_text_append_text(timeout_combo as *mut _, c_lbl.as_ptr());
    }
    let timeout_idx = CURL_TIMEOUT_INDEX.load(Ordering::SeqCst);
    gtk_combo_box_set_active(timeout_combo as *mut _, timeout_idx);
    let max_tokens_label = gtk_label_new(CString::new("Max tokens").unwrap().as_ptr());
    let max_tokens_combo = gtk_combo_box_text_new();
    gtk_widget_set_size_request(max_tokens_combo, 145, -1);
    gtk_widget_set_tooltip_text(
        max_tokens_combo,
        CString::new("Maximum generated tokens; choose Custom… for another value")
            .unwrap()
            .as_ptr(),
    );
    gtk_box_pack_start(limits_box as *mut _, timeout_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(limits_box as *mut _, timeout_combo, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(limits_box as *mut _, max_tokens_label, G_FALSE, G_FALSE, 8);
    gtk_box_pack_start(limits_box as *mut _, max_tokens_combo, G_FALSE, G_FALSE, 0);

    // Assembly into box
    gtk_box_pack_start(box_widget as *mut _, gtk_label_new(CString::new("Preset").unwrap().as_ptr()), G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, preset_box, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, name_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, name_entry, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, backend_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, radio_box, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, uri_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, uri_entry, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, uri_hint, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, model_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, model_box, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, api_key_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, api_key_entry, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, generation_box, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, insert_mode_label, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, insert_mode_combo, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, system_prompt_btn, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, thinking_log_check, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, thinking_log_loc_box, G_FALSE, G_FALSE, 0);
    gtk_box_pack_start(box_widget as *mut _, limits_box, G_FALSE, G_FALSE, 0);

    let mut widgets = Box::new(ConfigWidgets {
        plugin,
        configure_dialog: dialog,
        preset_combo,
        preset_name_entry: name_entry,
        ollama_radio,
        openai_compatible_radio: openai_radio,
        uri_entry,
        model_entry,
        api_key_entry,
        temperature_entry,
        language_hint_check,
        insert_mode_combo,
        delete_preset_button: delete_btn,
        timeout_combo,
        max_tokens_combo,
        thinking_log_check,
        thinking_log_sidebar_radio,
        thinking_log_msgwin_radio,
        max_token_values: Vec::new(),
        presets: Vec::new(),
        active_preset_index: 0,
        applying_preset: false,
    });

    with_global_state(|state| {
        widgets.presets = state.presets.clone();
        widgets.active_preset_index = ACTIVE_PRESET_INDEX.load(Ordering::SeqCst) as usize;
    });

    let widgets_ptr = Box::into_raw(widgets);

    refresh_preset_combo(&mut *widgets_ptr, (*widgets_ptr).active_preset_index);
    apply_preset_to_config_widgets(&mut *widgets_ptr, (*widgets_ptr).active_preset_index);
    refresh_max_tokens_combo(&mut *widgets_ptr, MAX_TOKENS.load(Ordering::SeqCst).max(0));

    let c_clicked = CString::new("clicked").unwrap();
    let c_changed = CString::new("changed").unwrap();
    let c_toggled = CString::new("toggled").unwrap();
    let c_response = CString::new("response").unwrap();

    g_signal_connect_data(
        thinking_log_check as GPointer,
        c_toggled.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_thinking_log_check_toggled)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        preset_combo as GPointer,
        c_changed.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkComboBox, GPointer), unsafe extern "C" fn()>(on_preset_changed)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        add_btn as GPointer,
        c_clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_add_preset_clicked)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        save_btn as GPointer,
        c_clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_save_preset_clicked)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        delete_btn as GPointer,
        c_clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_delete_preset_clicked)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        ollama_radio as GPointer,
        c_toggled.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(clear_model_on_upstream_change)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        openai_radio as GPointer,
        c_toggled.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(clear_model_on_upstream_change)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        uri_entry as GPointer,
        c_changed.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(clear_model_on_upstream_change)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        select_model_btn as GPointer,
        c_clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_select_model_clicked)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        system_prompt_btn as GPointer,
        c_clicked.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkWidget, GPointer), unsafe extern "C" fn()>(on_system_prompt_clicked)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    g_signal_connect_data(
        max_tokens_combo as GPointer,
        c_changed.as_ptr(),
        Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkComboBox, GPointer), unsafe extern "C" fn()>(on_max_tokens_changed)),
        widgets_ptr as GPointer,
        None,
        0,
    );

    if !dialog.is_null() {
        g_signal_connect_data(
            dialog as GPointer,
            c_response.as_ptr(),
            Some(std::mem::transmute::<unsafe extern "C" fn(*mut GtkDialog, c_int, GPointer), unsafe extern "C" fn()>(on_configure_response)),
            widgets_ptr as GPointer,
            Some(free_config_widgets),
            0,
        );
    } else {
        let c_destroy = CString::new("destroy").unwrap();
        g_signal_connect_data(
            box_widget as GPointer,
            c_destroy.as_ptr(),
            Some(std::mem::transmute::<
                unsafe extern "C" fn(*mut GtkWidget, GPointer),
                unsafe extern "C" fn(),
            >(on_settings_form_destroy)),
            widgets_ptr as GPointer,
            Some(free_config_widgets),
            0,
        );
    }

    gtk_widget_show_all(box_widget);
    box_widget
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fake_plugin_with_main_widgets;

    fn empty_widgets() -> ConfigWidgets {
        ConfigWidgets {
            plugin: ptr::null_mut(),
            configure_dialog: ptr::null_mut(),
            preset_combo: ptr::null_mut(),
            preset_name_entry: ptr::null_mut(),
            ollama_radio: ptr::null_mut(),
            openai_compatible_radio: ptr::null_mut(),
            uri_entry: ptr::null_mut(),
            model_entry: ptr::null_mut(),
            api_key_entry: ptr::null_mut(),
            temperature_entry: ptr::null_mut(),
            language_hint_check: ptr::null_mut(),
            insert_mode_combo: ptr::null_mut(),
            delete_preset_button: ptr::null_mut(),
            timeout_combo: ptr::null_mut(),
            max_tokens_combo: ptr::null_mut(),
            thinking_log_check: ptr::null_mut(),
            thinking_log_sidebar_radio: ptr::null_mut(),
            thinking_log_msgwin_radio: ptr::null_mut(),
            max_token_values: Vec::new(),
            presets: Vec::new(),
            active_preset_index: 0,
            applying_preset: false,
        }
    }

    #[test]
    fn text_helpers_tolerate_null_widgets() {
        unsafe {
            assert_eq!(safe_get_entry_text(ptr::null_mut()), "");
            assert_eq!(get_text_buffer_text(ptr::null_mut()), "");
            on_thinking_log_check_toggled(ptr::null_mut(), ptr::null_mut());
        }
    }

    #[test]
    fn dialog_parent_resolution_prefers_the_configure_dialog() {
        unsafe {
            let mut widgets = empty_widgets();
            // no dialog, no plugin
            assert!(config_dialog_parent(&widgets).is_null());

            // the dialog pointer is returned as-is, never dereferenced
            widgets.configure_dialog = 0x1000 as *mut GtkDialog;
            assert_eq!(config_dialog_parent(&widgets) as usize, 0x1000);

            // a plugin with a widget table but no window resolves to null
            widgets.configure_dialog = ptr::null_mut();
            let mut fake = fake_plugin_with_main_widgets(&std::env::temp_dir());
            widgets.plugin = fake.ptr();
            assert!(config_dialog_parent(&widgets).is_null());
        }
    }

    #[test]
    fn preset_sync_and_apply_ignore_out_of_range_indices() {
        unsafe {
            let mut widgets = empty_widgets();
            // empty preset list: both are no-ops and must not touch GTK
            sync_active_preset_from_widgets(&mut widgets);
            apply_preset_to_config_widgets(&mut widgets, 5);
            assert!(widgets.presets.is_empty());
            assert_eq!(widgets.active_preset_index, 0);
        }
    }

    #[test]
    fn widget_teardown_handles_null_and_owned_pointers() {
        unsafe {
            free_config_widgets(ptr::null_mut());
            let boxed = Box::new(empty_widgets());
            free_config_widgets(Box::into_raw(boxed) as GPointer);
            on_settings_form_destroy(ptr::null_mut(), ptr::null_mut());
        }
    }

    #[test]
    fn show_settings_dialog_handles_null_plugin() {
        unsafe {
            show_settings_dialog(ptr::null_mut());
        }
    }
}
