use crate::backend::{BackendPreset, BackendType, InsertMode, DEFAULT_CURL_TIMEOUT_INDEX, DEFAULT_OLLAMA_URI};
use std::sync::atomic::{AtomicI32, AtomicU64};
use std::sync::Mutex;

pub static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
pub static CURL_TIMEOUT_INDEX: AtomicI32 = AtomicI32::new(DEFAULT_CURL_TIMEOUT_INDEX as i32);
pub static MAX_TOKENS: AtomicI32 = AtomicI32::new(0);
pub const THINKING_LOG_LOCATION_SIDEBAR: i32 = 0;
pub const THINKING_LOG_LOCATION_MESSAGE_WINDOW: i32 = 1;

/// Whether decoded reasoning is retained and displayed.
/// Keeping this off avoids allocating a second copy of streamed reasoning.
pub static THINKING_LOG_ENABLED: AtomicI32 = AtomicI32::new(1);
/// Placement of the thinking log panel: 0 = Sidebar (default), 1 = Message window.
pub static THINKING_LOG_LOCATION: AtomicI32 = AtomicI32::new(THINKING_LOG_LOCATION_SIDEBAR);

pub const DEFAULT_THINKING_LOG_MSGWIN_SPLIT: i32 = 90;
/// Width proportion percentage (20..=95) for Column 1 when displayed in the message window.
pub static THINKING_LOG_MSGWIN_SPLIT: AtomicI32 = AtomicI32::new(DEFAULT_THINKING_LOG_MSGWIN_SPLIT);

pub static ACTIVE_PRESET_INDEX: AtomicI32 = AtomicI32::new(0);

// Thread-safe wrapper for global settings shared across main thread calls
pub struct GlobalState {
    pub presets: Vec<BackendPreset>,
    pub backend_type: BackendType,
    pub upstream_uri: String,
    pub model_name: String,
    pub system_prompt: String,
    pub api_key: String,
    pub temperature: String,
    pub include_language_hint: bool,
    pub insert_mode: InsertMode,
}

static GLOBAL_STATE: Mutex<Option<GlobalState>> = Mutex::new(None);

pub fn init_global_state() {
    let mut guard = GLOBAL_STATE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(GlobalState {
            presets: Vec::new(),
            backend_type: BackendType::Ollama,
            upstream_uri: DEFAULT_OLLAMA_URI.to_string(),
            model_name: String::new(),
            system_prompt: String::new(),
            api_key: String::new(),
            temperature: String::new(),
            include_language_hint: true,
            insert_mode: InsertMode::Cursor,
        });
    }
}

pub fn with_global_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut GlobalState) -> R,
{
    init_global_state();
    let mut guard = GLOBAL_STATE.lock().unwrap();
    f(guard.as_mut().unwrap())
}

#[cfg(test)]
static TEST_GLOBALS_LOCK: Mutex<()> = Mutex::new(());

/// Tests that touch process-wide state (the atomics, `GLOBAL_STATE`, or
/// `request::ACTIVE_REQUEST`) take this lock so they cannot interleave.
#[cfg(test)]
pub fn test_globals_guard() -> std::sync::MutexGuard<'static, ()> {
    TEST_GLOBALS_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_state_initializes_once_and_keeps_mutations() {
        let _guard = test_globals_guard();
        let original = with_global_state(|state| state.model_name.clone());
        with_global_state(|state| state.model_name = "coverage-model".to_string());
        init_global_state(); // must not reset already-initialized state
        assert_eq!(
            with_global_state(|state| state.model_name.clone()),
            "coverage-model"
        );
        with_global_state(|state| state.model_name = original);
    }
}
