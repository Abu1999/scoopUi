use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub active_root: Mutex<Option<PathBuf>>,
    pub tasks: Mutex<HashMap<String, u32>>,
}
