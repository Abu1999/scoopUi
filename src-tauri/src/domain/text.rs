//! 文本读取容错。
//!
//! Scoop 与 PowerShell 写出的元数据文件编码很杂：
//!
//! - `Set-Content -Encoding UTF8`（Windows PowerShell 5.1）→ UTF-8 **带 BOM**
//! - `Out-File` / `Set-Content` 默认 → UTF-16LE 带 BOM
//! - 部分分支脚本 → 系统 ANSI（中文机器为 GBK）
//!
//! 直接 `read_to_string` + `serde_json::from_str` 在这些文件上会失败，
//! 结果是已安装应用、配置集、软件源清单被静默「识别不到」。
//! 这里统一做编码嗅探 + BOM 剥离，让调用方按纯文本处理。

use std::path::Path;

/// 解码文件字节为文本：识别 UTF-8 / UTF-16 BOM，其余按 UTF-8 有损解码。
///
/// 有损解码是有意为之：JSON 的结构字符都是 ASCII，即使字符串内容出现
/// 非 UTF-8 字节（如 GBK 中文描述），也只会损失显示文本而不会丢掉整条记录。
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, false);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        units.push(if little_endian {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
        });
    }
    String::from_utf16_lossy(&units)
}

/// 去掉开头的 BOM 与空白字符。
pub fn strip_bom(text: &str) -> &str {
    text.trim_start_matches('\u{feff}').trim_start()
}

/// 读取文件并解码为文本，失败返回 None。
pub fn read_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(decode(&bytes))
}

/// 读取文件并按编码/BOM 容错解析为 JSON，失败返回 None。
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    parse_json(&read_text(path)?)
}

/// 按 BOM 容错解析 JSON 文本，失败返回 None。
pub fn parse_json<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_str(strip_bom(text)).ok()
}

/// 按 BOM 容错解析 JSON 文本，失败给出 `serde_json` 的错误。
pub fn parse_json_result<T: serde::de::DeserializeOwned>(
    text: &str,
) -> Result<T, serde_json::Error> {
    serde_json::from_str(strip_bom(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le_bytes(text: &str) -> Vec<u8> {
        let mut out = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out
    }

    #[test]
    fn decodes_plain_utf8() {
        assert_eq!(decode(br#"{"a":"b"}"#), r#"{"a":"b"}"#);
    }

    #[test]
    fn decodes_utf8_with_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(r#"{"a":"中文"}"#.as_bytes());
        assert_eq!(decode(&bytes), r#"{"a":"中文"}"#);
    }

    #[test]
    fn decodes_utf16_both_endians() {
        assert_eq!(decode(&utf16le_bytes(r#"{"a":1}"#)), r#"{"a":1}"#);
        let mut be = vec![0xFE, 0xFF];
        for unit in r#"{"a":1}"#.encode_utf16() {
            be.extend_from_slice(&unit.to_be_bytes());
        }
        assert_eq!(decode(&be), r#"{"a":1}"#);
    }

    #[test]
    fn lossy_decodes_non_utf8_without_dropping_structure() {
        let mut bytes = br#"{"description":""#.to_vec();
        bytes.extend_from_slice(&[0xD6, 0xD0, 0xCE, 0xC4]); // GBK「中文」
        bytes.extend_from_slice(br#""}"#);
        let text = decode(&bytes);
        assert!(text.starts_with(r#"{"description":"#));
        assert!(text.ends_with(r#""}"#));
    }

    #[test]
    fn strip_bom_removes_leading_zero_width_and_space() {
        assert_eq!(strip_bom("\u{feff}  {\"a\":1}"), "{\"a\":1}");
        assert_eq!(strip_bom("{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn parse_json_tolerates_bom_and_reports_errors() {
        let value: Option<serde_json::Value> = parse_json("\u{feff}{\"a\":1}");
        assert_eq!(value.unwrap()["a"], 1);
        let bad: Option<serde_json::Value> = parse_json("{oops");
        assert!(bad.is_none());
        assert!(parse_json_result::<serde_json::Value>("{oops").is_err());
    }

    #[test]
    fn read_json_reads_bom_encoded_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("manifest.json");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(br#"{"version":"1.0"}"#);
        std::fs::write(&path, bytes).unwrap();
        let value: serde_json::Value = read_json(&path).unwrap();
        assert_eq!(value["version"], "1.0");
        assert!(read_json::<serde_json::Value>(&tmp.path().join("nope.json")).is_none());
    }
}
