use serde::ser::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("未找到可用的 scoop 根目录")]
    NoRoot,
    #[error("根目录无效（缺少 apps/buckets/shims 目录）: {0}")]
    InvalidRoot(String),
    #[error("未找到 scoop 命令行脚本: {0}")]
    MissingScoopScript(String),
    #[error("操作失败: {0}")]
    Io(String),
    #[error("JSON 解析错误: {0}")]
    Json(String),
    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Json(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_chinese_message() {
        let v = serde_json::to_string(&AppError::NoRoot).unwrap();
        assert_eq!(v, "\"未找到可用的 scoop 根目录\"");
    }

    #[test]
    fn serializes_missing_script_with_chinese_prefix() {
        let v = serde_json::to_string(&AppError::MissingScoopScript("C:\\scoop".into())).unwrap();
        assert!(v.starts_with("\"未找到 scoop 命令行脚本"), "实际: {}", v);
    }

    #[test]
    fn converts_io_error() {
        let e: AppError = std::io::Error::new(std::io::ErrorKind::NotFound, "x").into();
        assert!(e.to_string().contains("操作失败"));
    }
}
