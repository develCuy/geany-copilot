pub mod backend;
pub mod config;
pub mod configure;
pub mod ffi;
pub mod globals;
pub mod request;
#[cfg(test)]
pub mod test_support;
pub mod ui;

use config::{load_config, save_config};
use configure::copilot_plugin_configure;
use ffi::geany::*;
use ffi::glib::*;
use ffi::gtk::*;
use request::abandon_active_request;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
use ui::*;

static PLUGIN_NAME: &[u8] = b"Geany Copilot\0";
static PLUGIN_DESC: &[u8] = b"Reads 100 characters before and after the cursor and asks Copilot via the configured backend.\0";
static PLUGIN_VERSION: &[u8] = b"1.0\0";
static PLUGIN_AUTHOR: &[u8] = b"Developer\0";

unsafe extern "C" fn copilot_plugin_init(plugin: *mut GeanyPlugin, _user_data: GPointer) -> GBoolean {
    load_config(plugin);

    install_statusbar_preset_combo(plugin);
    set_thinking_log_enabled(
        plugin,
        globals::THINKING_LOG_ENABLED.load(std::sync::atomic::Ordering::SeqCst) != 0,
    );

    let key_group = plugin_set_key_group_full(
        plugin,
        CString::new("geany_copilot").unwrap().as_ptr(),
        2,
        Some(on_keybinding),
        plugin as GPointer,
        None,
    );

    keybindings_set_item(
        key_group,
        0,
        None,
        0,
        0,
        CString::new("ask_copilot").unwrap().as_ptr(),
        CString::new("Ask Copilot").unwrap().as_ptr(),
        ptr::null_mut(),
    );

    keybindings_set_item(
        key_group,
        1,
        None,
        0,
        0,
        CString::new("switch_preset").unwrap().as_ptr(),
        CString::new("Next Copilot Preset").unwrap().as_ptr(),
        ptr::null_mut(),
    );

    let icon = gtk_image_new_from_icon_name(
        CString::new("system-run").unwrap().as_ptr(),
        GTK_ICON_SIZE_BUTTON,
    );
    let tool_button = gtk_tool_button_new(icon, CString::new("Ask Copilot").unwrap().as_ptr());
    gtk_widget_show_all(tool_button as *mut _);

    let c_signal = CString::new("clicked").unwrap();
    g_signal_connect_data(
        tool_button as GPointer,
        c_signal.as_ptr(),
        Some(std::mem::transmute::<
            unsafe extern "C" fn(*mut GtkWidget, GPointer),
            unsafe extern "C" fn(),
        >(on_tool_button_clicked)),
        plugin as GPointer,
        None,
        0,
    );

    if !P_DATA.is_null() {
        (*P_DATA).tool_button = tool_button as *mut _;
    }

    let main_widgets = (*(*plugin).geany_data).main_widgets;
    if main_widgets.is_null() { return G_FALSE; }
    let toolbar = (*main_widgets).toolbar;
    if !toolbar.is_null() {
        gtk_toolbar_insert(toolbar as *mut _, tool_button, -1);
    }

    G_TRUE
}

unsafe extern "C" fn copilot_plugin_cleanup(_plugin: *mut GeanyPlugin, _user_data: GPointer) {
    abandon_active_request();
    save_config(_plugin);

    if !P_DATA.is_null() {
        let pd = Box::from_raw(P_DATA);
        if !pd.statusbar_timeout_box.is_null() {
            gtk_widget_destroy(pd.statusbar_timeout_box);
        }
        if !pd.statusbar_max_tokens_box.is_null() {
            gtk_widget_destroy(pd.statusbar_max_tokens_box);
        }
        if !pd.statusbar_preset_box.is_null() {
            gtk_widget_destroy(pd.statusbar_preset_box);
        }
        set_thinking_log_enabled(_plugin, false);
        if !pd.tool_button.is_null() {
            gtk_widget_destroy(pd.tool_button);
        }
        P_DATA = ptr::null_mut();
    }
}

#[no_mangle]
pub unsafe extern "C" fn geany_load_module(plugin: *mut GeanyPlugin) {
    let info = (*plugin).info;
    (*info).name = PLUGIN_NAME.as_ptr() as *const c_char;
    (*info).description = PLUGIN_DESC.as_ptr() as *const c_char;
    (*info).version = PLUGIN_VERSION.as_ptr() as *const c_char;
    (*info).author = PLUGIN_AUTHOR.as_ptr() as *const c_char;

    let funcs = (*plugin).funcs;
    (*funcs).init = Some(copilot_plugin_init);
    (*funcs).configure = Some(copilot_plugin_configure);
    (*funcs).cleanup = Some(copilot_plugin_cleanup);

    plugin_module_make_resident(plugin);
    geany_plugin_register(plugin, GEANY_API_VERSION, GEANY_API_VERSION, GEANY_ABI_VERSION);
}
