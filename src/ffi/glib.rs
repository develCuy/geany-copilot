use std::os::raw::{c_char, c_int, c_longlong, c_uint, c_void};

pub type GBoolean = c_int;
pub type GChar = c_char;
pub type GInt = c_int;
pub type GUint = c_uint;
pub type GInt64 = c_longlong;
pub type GDouble = std::os::raw::c_double;
pub type GPointer = *mut c_void;

pub type GCallback = Option<unsafe extern "C" fn()>;
pub type GDestroyNotify = Option<unsafe extern "C" fn(GPointer)>;
pub type GSourceFunc = Option<unsafe extern "C" fn(GPointer) -> GBoolean>;

pub const G_TRUE: GBoolean = 1;
pub const G_FALSE: GBoolean = 0;

pub const G_KEY_FILE_NONE: c_int = 0;

#[repr(C)]
pub struct GKeyFile {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct GError {
    pub domain: u32,
    pub code: c_int,
    pub message: *mut GChar,
}

#[repr(C)]
pub struct GDateTime {
    _opaque: [u8; 0],
}

extern "C" {
    pub fn g_free(mem: GPointer);

    pub fn g_get_monotonic_time() -> GInt64;
    pub fn g_idle_add(function: GSourceFunc, data: GPointer) -> GUint;
    pub fn g_date_time_new_now_local() -> *mut GDateTime;
    pub fn g_date_time_format(datetime: *mut GDateTime, format: *const GChar) -> *mut GChar;
    pub fn g_date_time_unref(datetime: *mut GDateTime);

    pub fn g_key_file_new() -> *mut GKeyFile;
    pub fn g_key_file_free(key_file: *mut GKeyFile);
    pub fn g_key_file_load_from_file(
        key_file: *mut GKeyFile,
        file: *const GChar,
        flags: c_int,
        error: *mut *mut GError,
    ) -> GBoolean;
    pub fn g_key_file_save_to_file(
        key_file: *mut GKeyFile,
        filename: *const GChar,
        error: *mut *mut GError,
    ) -> GBoolean;
    pub fn g_key_file_has_key(
        key_file: *mut GKeyFile,
        group_name: *const GChar,
        key: *const GChar,
        error: *mut *mut GError,
    ) -> GBoolean;
    pub fn g_key_file_get_string(
        key_file: *mut GKeyFile,
        group_name: *const GChar,
        key: *const GChar,
        error: *mut *mut GError,
    ) -> *mut GChar;
    pub fn g_key_file_set_string(
        key_file: *mut GKeyFile,
        group_name: *const GChar,
        key: *const GChar,
        string: *const GChar,
    );
    pub fn g_key_file_get_integer(
        key_file: *mut GKeyFile,
        group_name: *const GChar,
        key: *const GChar,
        error: *mut *mut GError,
    ) -> GInt;
    pub fn g_key_file_set_integer(
        key_file: *mut GKeyFile,
        group_name: *const GChar,
        key: *const GChar,
        value: GInt,
    );

    pub fn g_get_user_config_dir() -> *const GChar;
    pub fn g_build_filename(first_element: *const GChar, ...) -> *mut GChar;
    pub fn g_mkdir_with_parents(pathname: *const GChar, mode: c_int) -> c_int;
    pub fn g_error_free(error: *mut GError);
}

#[link(name = "gobject-2.0")]
extern "C" {
    pub fn g_object_ref(object: GPointer) -> GPointer;
    pub fn g_object_unref(object: GPointer);
}
