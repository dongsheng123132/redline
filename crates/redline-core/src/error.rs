//! 统一错误类型。
//!
//! 每个错误自带一个稳定的 `code`（给机器看）和一个退出码分类（给 shell 看）。
//! 三个界面（CLI / GUI / MCP）共用同一套错误，不各自造一套文案。

use std::fmt;

/// 错误分类 —— 直接决定 CLI 的退出码，含义对外稳定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// 输入不对：文件不存在、格式不支持、patch 结构非法。退出码 1。
    Input,
    /// 安全拒绝：会覆盖原件、原件已变更、替换点不唯一。退出码 2。
    /// 这一类**不是 bug**，是核心主动拒绝执行，调用方应当原样呈现给人看。
    Refused,
    /// 内部错误：解析崩了、IO 失败。退出码 3。
    Internal,
}

impl ErrorClass {
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorClass::Input => 1,
            ErrorClass::Refused => 2,
            ErrorClass::Internal => 3,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorClass::Input => "input",
            ErrorClass::Refused => "refused",
            ErrorClass::Internal => "internal",
        }
    }
}

#[derive(Debug)]
pub struct RedlineError {
    pub class: ErrorClass,
    /// 稳定的机器可读错误码，例如 `unsupported_format`、`output_would_overwrite_input`。
    pub code: &'static str,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl RedlineError {
    pub fn new(class: ErrorClass, code: &'static str, message: impl Into<String>) -> Self {
        Self { class, code, message: message.into(), details: None }
    }

    pub fn input(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Input, code, message)
    }

    pub fn refused(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Refused, code, message)
    }

    pub fn internal(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Internal, code, message)
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// 序列化成信封里的 `error` 字段。
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("class".into(), self.class.as_str().into());
        map.insert("code".into(), self.code.into());
        map.insert("message".into(), self.message.clone().into());
        if let Some(details) = &self.details {
            map.insert("details".into(), details.clone());
        }
        serde_json::Value::Object(map)
    }
}

impl fmt::Display for RedlineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RedlineError {}

pub type Result<T> = std::result::Result<T, RedlineError>;

impl From<std::io::Error> for RedlineError {
    fn from(err: std::io::Error) -> Self {
        let class = if err.kind() == std::io::ErrorKind::NotFound {
            ErrorClass::Input
        } else {
            ErrorClass::Internal
        };
        RedlineError::new(class, "io_error", err.to_string())
    }
}

impl From<zip::result::ZipError> for RedlineError {
    fn from(err: zip::result::ZipError) -> Self {
        RedlineError::input("bad_archive", format!("压缩包读取失败：{err}"))
    }
}

impl From<quick_xml::Error> for RedlineError {
    fn from(err: quick_xml::Error) -> Self {
        RedlineError::internal("bad_xml", format!("XML 解析失败：{err}"))
    }
}

impl From<quick_xml::events::attributes::AttrError> for RedlineError {
    fn from(err: quick_xml::events::attributes::AttrError) -> Self {
        RedlineError::internal("bad_xml_attribute", format!("XML 属性解析失败：{err}"))
    }
}

impl From<serde_json::Error> for RedlineError {
    fn from(err: serde_json::Error) -> Self {
        RedlineError::input("bad_json", format!("JSON 解析失败：{err}"))
    }
}
