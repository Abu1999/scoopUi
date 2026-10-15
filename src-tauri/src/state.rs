use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::domain::index::PackageEntry;

pub struct IndexCache {
    pub root: PathBuf,
    pub entries: Arc<Vec<PackageEntry>>,
}

/// 任务作用域：决定并发规则。
///
/// - `Global`：会动到全环境（更新全部/软件源增删/维护/自更新等）——独占运行，
///   且任何其它任务运行期间都不能启动；
/// - `App(name)`：只作用于单个应用（安装/更新/卸载/锁定）——不同应用可并发，
///   同应用或与 `Global` 互斥。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskScope {
    Global,
    App(String),
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub pid: u32,
    pub scope: TaskScope,
}

#[derive(Default)]
pub struct AppState {
    pub scoop_dir: Mutex<Option<PathBuf>>,
    pub tasks: Mutex<HashMap<String, TaskInfo>>,
    pub index: Mutex<Option<IndexCache>>,
}
