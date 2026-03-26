use serde::Deserialize;
use serde_json::Value;

/// JSONL の各行型
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LogRecord {
    #[serde(rename = "assistant")]
    Assistant(AssistantRecord),
    #[serde(rename = "user")]
    User(UserRecord),
    #[serde(rename = "system")]
    System(SystemRecord),
    #[serde(rename = "progress")]
    Progress(()),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(()),
}

/// system レコード — システムプロンプトやコンテキスト圧縮イベントを含む
#[derive(Debug, Deserialize)]
pub struct SystemRecord {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    /// コンテキスト圧縮時に設定される ("context_compression" 等)
    pub subtype: Option<String>,
    /// 圧縮後のサマリー文字列（圧縮イベント時のみ）
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssistantRecord {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "gitBranch")]
    pub git_branch: Option<String>,
    pub cwd: Option<String>,
    pub entrypoint: Option<String>,
    pub version: Option<String>,
    pub message: AssistantMessage,
}

#[derive(Debug, Deserialize)]
pub struct AssistantMessage {
    pub model: Option<String>,
    pub usage: Option<Usage>,
    pub content: Option<Vec<ContentBlock>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Usage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_input_tokens: Option<i64>,
    pub cache_read_input_tokens: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    #[allow(dead_code)]
    Text {
        text: Option<String>,
    },
    #[allow(dead_code)]
    Thinking {
        thinking: Option<String>,
    },
    ToolUse(ToolUseBlock),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToolUseBlock {
    pub id: Option<String>,
    pub name: Option<String>,
    #[allow(dead_code)]
    pub input: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct UserRecord {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    #[serde(rename = "gitBranch")]
    pub git_branch: Option<String>,
    pub cwd: Option<String>,
    pub entrypoint: Option<String>,
    pub version: Option<String>,
    pub message: UserMessage,
}

#[derive(Debug, Deserialize)]
pub struct UserMessage {
    #[allow(dead_code)]
    pub role: Option<String>,
    pub content: Option<UserContent>,
}

/// content は文字列の場合とブロック配列の場合がある
/// serde(untagged) の順序に注意: Blocks を先に試みる
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    Blocks(Vec<UserContentBlock>),
    #[allow(dead_code)]
    Text(String),
}

#[derive(Debug, Deserialize)]
pub struct UserContentBlock {
    #[serde(rename = "type")]
    pub block_type: Option<String>,
    pub tool_use_id: Option<String>,
    pub is_error: Option<bool>,
    #[allow(dead_code)]
    pub content: Option<Value>,
}
