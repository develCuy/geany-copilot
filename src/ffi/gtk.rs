use crate::ffi::glib::{GBoolean, GCallback, GChar, GDestroyNotify, GDouble, GPointer};
use std::os::raw::{c_int, c_ulong, c_void};

pub const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
pub const GTK_ORIENTATION_VERTICAL: c_int = 1;

pub const GTK_RESPONSE_OK: c_int = -5;
pub const GTK_RESPONSE_CANCEL: c_int = -6;
pub const GTK_RESPONSE_APPLY: c_int = -10;
pub const GTK_DIALOG_MODAL: c_int = 1;
pub const GTK_POLICY_AUTOMATIC: c_int = 0;

pub const GTK_MESSAGE_ERROR: c_int = 3;

pub const GTK_BUTTONS_CLOSE: c_int = 2;

pub const GTK_ICON_SIZE_BUTTON: c_int = 4;
pub const GTK_WRAP_WORD_CHAR: c_int = 3;

pub type GtkWidget = c_void;
pub type GtkDialog = c_void;
pub type GtkWindow = c_void;
pub type GtkBox = c_void;
pub type GtkLabel = c_void;
pub type GtkEntry = c_void;
pub type GtkRadioButton = c_void;
pub type GtkComboBox = c_void;
pub type GtkComboBoxText = c_void;
pub type GtkNotebook = c_void;
pub type GtkToolItem = c_void;
pub type GtkToolbar = c_void;
pub type GtkTextView = c_void;
pub type GtkTextBuffer = c_void;
pub type GtkTextMark = c_void;
pub type GtkContainer = c_void;
pub type GtkScrolledWindow = c_void;
pub type GtkPaned = c_void;
pub type PangoFontDescription = c_void;

#[repr(C)]
pub struct GdkRGBA {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

#[repr(C)]
pub struct GtkAllocation {
    pub x: c_int,
    pub y: c_int,
    pub width: c_int,
    pub height: c_int,
}

pub const GTK_STATE_FLAG_NORMAL: c_int = 0;
pub const GTK_STATE_FLAG_INSENSITIVE: c_int = 1 << 3;

#[repr(C)]
pub struct GtkTextIter {
    dummy1: *mut c_void,
    dummy2: *mut c_void,
    dummy3: c_int,
    dummy4: c_int,
    dummy5: c_int,
    dummy6: c_int,
    dummy7: c_int,
    dummy8: c_int,
    dummy9: *mut c_void,
    dummy10: *mut c_void,
    dummy11: c_int,
    dummy12: c_int,
    dummy13: c_int,
    dummy14: *mut c_void,
}

extern "C" {
    pub fn gtk_widget_show_all(widget: *mut GtkWidget);
    pub fn gtk_widget_destroy(widget: *mut GtkWidget);
    pub fn gtk_widget_get_parent(widget: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_get_allocated_width(widget: *mut GtkWidget) -> c_int;
    pub fn gtk_widget_set_sensitive(widget: *mut GtkWidget, sensitive: GBoolean);
    pub fn gtk_widget_set_hexpand(widget: *mut GtkWidget, expand: GBoolean);
    pub fn gtk_widget_set_vexpand(widget: *mut GtkWidget, expand: GBoolean);
    pub fn gtk_widget_set_margin_start(widget: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_end(widget: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_top(widget: *mut GtkWidget, margin: c_int);
    pub fn gtk_widget_set_margin_bottom(widget: *mut GtkWidget, margin: c_int);

    pub fn gtk_container_add(container: *mut GtkContainer, widget: *mut GtkWidget);
    pub fn gtk_container_remove(container: *mut GtkContainer, widget: *mut GtkWidget);

    pub fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut GtkWidget;
    pub fn gtk_box_pack_start(
        box_: *mut GtkBox,
        child: *mut GtkWidget,
        expand: GBoolean,
        fill: GBoolean,
        padding: c_int,
    );

    pub fn gtk_label_new(str: *const GChar) -> *mut GtkWidget;
    pub fn gtk_label_set_text(label: *mut GtkLabel, str: *const GChar);
    pub fn gtk_label_set_line_wrap(label: *mut GtkLabel, wrap: GBoolean);
    pub fn gtk_label_set_xalign(label: *mut GtkLabel, xalign: f32);

    pub fn gtk_entry_new() -> *mut GtkWidget;
    pub fn gtk_entry_get_text(entry: *mut GtkEntry) -> *const GChar;
    pub fn gtk_entry_set_text(entry: *mut GtkEntry, text: *const GChar);
    pub fn gtk_entry_set_placeholder_text(entry: *mut GtkEntry, text: *const GChar);
    pub fn gtk_entry_set_visibility(entry: *mut GtkEntry, visible: GBoolean);

    pub fn gtk_button_new_with_label(label: *const GChar) -> *mut GtkWidget;
    pub fn gtk_check_button_new_with_label(label: *const GChar) -> *mut GtkWidget;
    pub fn gtk_radio_button_new_with_label_from_widget(
        radio_group_member: *mut GtkRadioButton,
        label: *const GChar,
    ) -> *mut GtkWidget;
    pub fn gtk_toggle_button_get_active(toggle_button: *mut GtkWidget) -> GBoolean;
    pub fn gtk_toggle_button_set_active(toggle_button: *mut GtkWidget, is_active: GBoolean);

    pub fn gtk_combo_box_text_new() -> *mut GtkWidget;
    pub fn gtk_combo_box_text_append_text(combo_box: *mut GtkComboBoxText, text: *const GChar);
    pub fn gtk_combo_box_text_remove_all(combo_box: *mut GtkComboBoxText);
    pub fn gtk_combo_box_set_active(combo_box: *mut GtkComboBox, index: c_int);
    pub fn gtk_combo_box_get_active(combo_box: *mut GtkComboBox) -> c_int;

    pub fn gtk_notebook_new() -> *mut GtkWidget;
    pub fn gtk_notebook_append_page(
        notebook: *mut GtkNotebook,
        child: *mut GtkWidget,
        tab_label: *mut GtkWidget,
    ) -> c_int;
    pub fn gtk_notebook_set_current_page(notebook: *mut GtkNotebook, page_num: c_int);
    pub fn gtk_notebook_page_num(notebook: *mut GtkNotebook, child: *mut GtkWidget) -> c_int;

    pub fn gtk_paned_new(orientation: c_int) -> *mut GtkWidget;
    pub fn gtk_paned_pack1(
        paned: *mut GtkPaned,
        child: *mut GtkWidget,
        resize: GBoolean,
        shrink: GBoolean,
    );
    pub fn gtk_paned_pack2(
        paned: *mut GtkPaned,
        child: *mut GtkWidget,
        resize: GBoolean,
        shrink: GBoolean,
    );
    pub fn gtk_paned_get_child2(paned: *mut GtkPaned) -> *mut GtkWidget;
    pub fn gtk_paned_set_position(paned: *mut GtkPaned, position: c_int);
    pub fn gtk_paned_get_position(paned: *mut GtkPaned) -> c_int;

    pub fn gtk_image_new_from_icon_name(
        icon_name: *const GChar,
        size: c_int,
    ) -> *mut GtkWidget;
    pub fn gtk_tool_button_new(
        icon_widget: *mut GtkWidget,
        label: *const GChar,
    ) -> *mut GtkToolItem;
    pub fn gtk_toolbar_insert(
        toolbar: *mut GtkToolbar,
        item: *mut GtkToolItem,
        pos: c_int,
    );

    pub fn gtk_scrolled_window_new(
        hadjustment: GPointer,
        vadjustment: GPointer,
    ) -> *mut GtkWidget;

    pub fn gtk_text_view_new() -> *mut GtkWidget;
    pub fn gtk_text_view_get_buffer(text_view: *mut GtkTextView) -> *mut GtkTextBuffer;
    pub fn gtk_text_view_set_editable(text_view: *mut GtkTextView, setting: GBoolean);
    pub fn gtk_text_view_set_cursor_visible(text_view: *mut GtkTextView, setting: GBoolean);
    pub fn gtk_text_view_set_wrap_mode(text_view: *mut GtkTextView, wrap_mode: c_int);
    pub fn gtk_text_view_set_monospace(text_view: *mut GtkTextView, monospace: GBoolean);

    pub fn gtk_text_buffer_insert(
        buffer: *mut GtkTextBuffer,
        iter: *mut GtkTextIter,
        text: *const GChar,
        len: c_int,
    );
    pub fn gtk_text_buffer_set_text(
        buffer: *mut GtkTextBuffer,
        text: *const GChar,
        len: c_int,
    );
    pub fn gtk_text_buffer_get_bounds(
        buffer: *mut GtkTextBuffer,
        start: *mut GtkTextIter,
        end: *mut GtkTextIter,
    );
    pub fn gtk_text_buffer_get_end_iter(buffer: *mut GtkTextBuffer, iter: *mut GtkTextIter);
    pub fn gtk_text_buffer_get_text(
        buffer: *mut GtkTextBuffer,
        start: *const GtkTextIter,
        end: *const GtkTextIter,
        include_hidden_chars: GBoolean,
    ) -> *mut GChar;
    pub fn gtk_text_buffer_create_mark(
        buffer: *mut GtkTextBuffer,
        mark_name: *const GChar,
        where_: *const GtkTextIter,
        left_gravity: GBoolean,
    ) -> *mut GtkTextMark;
    pub fn gtk_text_buffer_delete_mark(buffer: *mut GtkTextBuffer, mark: *mut GtkTextMark);
    pub fn gtk_text_view_scroll_to_mark(
        text_view: *mut GtkTextView,
        mark: *mut GtkTextMark,
        within_margin: GDouble,
        use_align: GBoolean,
        xalign: GDouble,
        yalign: GDouble,
    );
    pub fn gtk_dialog_new_with_buttons(
        title: *const GChar,
        parent: *mut GtkWindow,
        flags: c_int,
        first_button_text: *const GChar,
        ...
    ) -> *mut GtkWidget;
    pub fn gtk_dialog_run(dialog: *mut GtkDialog) -> c_int;
    pub fn gtk_dialog_get_content_area(dialog: *mut GtkDialog) -> *mut GtkWidget;

    pub fn gtk_message_dialog_new(
        parent: *mut GtkWindow,
        flags: c_int,
        type_: c_int,
        buttons: c_int,
        message_format: *const GChar,
        ...
    ) -> *mut GtkWidget;

    pub fn gtk_window_set_default_size(window: *mut GtkWindow, width: c_int, height: c_int);
    pub fn gtk_window_set_destroy_with_parent(window: *mut GtkWindow, setting: GBoolean);

    pub fn gtk_statusbar_get_message_area(statusbar: *mut GtkWidget) -> *mut GtkWidget;
    pub fn gtk_widget_set_size_request(widget: *mut GtkWidget, width: c_int, height: c_int);
    pub fn gtk_widget_set_tooltip_text(widget: *mut GtkWidget, text: *const GChar);
    pub fn gtk_widget_override_color(
        widget: *mut GtkWidget,
        state: c_int,
        color: *const GdkRGBA,
    );
    pub fn gtk_widget_override_background_color(
        widget: *mut GtkWidget,
        state: c_int,
        color: *const GdkRGBA,
    );
    pub fn gtk_widget_override_font(
        widget: *mut GtkWidget,
        font_desc: *const PangoFontDescription,
    );
    pub fn gtk_scrolled_window_set_policy(window: *mut GtkScrolledWindow, hscrollbar_policy: c_int, vscrollbar_policy: c_int);
    pub fn gtk_scrolled_window_set_min_content_width(window: *mut GtkScrolledWindow, width: c_int);
    pub fn gtk_scrolled_window_set_min_content_height(window: *mut GtkScrolledWindow, height: c_int);

    pub fn g_signal_connect_data(
        instance: GPointer,
        detailed_signal: *const GChar,
        c_handler: GCallback,
        data: GPointer,
        destroy_data: GDestroyNotify,
        connect_flags: c_int,
    ) -> c_ulong;
}

#[link(name = "pango-1.0")]
extern "C" {
    pub fn pango_font_description_new() -> *mut PangoFontDescription;
    pub fn pango_font_description_free(desc: *mut PangoFontDescription);
    pub fn pango_font_description_set_family(
        desc: *mut PangoFontDescription,
        family: *const GChar,
    );
}
