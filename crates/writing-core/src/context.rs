//! Protocol-independent context planning and provider result types.
use crate::{AnalysisRecord, ConfigRef, ContextCapability, ConversationContext};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub fn protocol(c: &ConfigRef) -> &str {
    c.api_protocol.as_deref().unwrap_or("chat_completions")
}
pub fn policy(c: &ConfigRef) -> &str {
    c.context_policy.as_deref().unwrap_or("local")
}
pub fn fingerprint(c: &ConfigRef, key: &str) -> String {
    digest(&json!([c.base_url, c.model_name, protocol(c), key]))
}
pub fn digest(value: &impl Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("serializable context"))
    )
}
pub fn remote_enabled(c: &ConfigRef, key: &str) -> crate::Result<bool> {
    let supported = protocol(c) == "responses"
        && c.context_capability
            .as_ref()
            .is_some_and(|cap| cap.status == "supported" && cap.fingerprint == fingerprint(c, key));
    if policy(c) == "remote_required" && !supported {
        return Err(
            "此连接尚未验证服务端上下文接续，请先测试，或将上下文管理改为自动/本地。".into(),
        );
    }
    Ok(policy(c) != "local" && supported)
}
pub fn binding(c: &ConfigRef, key: &str, prompt_version: &str, r: &AnalysisRecord) -> String {
    digest(&json!([
        c.id,
        fingerprint(c, key),
        prompt_version,
        r.sources,
        r.question_hashes
    ]))
}
pub fn history_digest(r: &AnalysisRecord) -> String {
    digest(&json!([
        r.sources,
        r.question_hashes,
        r.opening_message,
        r.markdown,
        r.messages
            .iter()
            .map(|m| (&m.role, &m.content))
            .collect::<Vec<_>>()
    ]))
}
pub fn history(r: &AnalysisRecord) -> Vec<Value> {
    let mut result = vec![];
    if let Some(opening) = &r.opening_message {
        result.push(json!({"role":"user","content":opening}));
    }
    result.push(json!({"role":"assistant","content":r.markdown}));
    result.extend(
        r.messages
            .iter()
            .map(|m| json!({"role":m.role,"content":m.content})),
    );
    result
}
pub fn text_size(message: &Value) -> usize {
    match message.get("content") {
        Some(Value::String(s)) => s.chars().count(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .map(|s| s.chars().count())
            .sum(),
        _ => 0,
    }
}
pub fn can_continue(state: &ConversationContext, binding: &str, r: &AnalysisRecord) -> bool {
    state.binding_fingerprint == binding && state.history_digest == history_digest(r)
}
pub fn capability(
    status: &str,
    tested_at: String,
    fp: String,
    reason: Option<String>,
) -> ContextCapability {
    ContextCapability {
        status: status.into(),
        tested_at,
        fingerprint: fp,
        reason,
    }
}

#[derive(Clone, Debug)]
pub struct GenerationRequest {
    pub messages: Vec<Value>,
    pub previous_response_id: Option<String>,
    pub store: bool,
}
impl GenerationRequest {
    pub fn stateless(messages: Vec<Value>) -> Self {
        Self {
            messages,
            previous_response_id: None,
            store: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationResult {
    pub text: String,
    pub response_id: Option<String>,
    pub output_items: Vec<Value>,
    pub usage: Option<Value>,
    pub provider_request_id: Option<String>,
}
impl GenerationResult {
    pub fn text(text: String) -> Self {
        Self {
            text,
            response_id: None,
            output_items: vec![],
            usage: None,
            provider_request_id: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum GatewayErrorKind {
    UnsupportedContinuation,
    RemoteStateMissing,
    Unauthorized,
    RateLimited,
    ContextExceeded,
    OutcomeUnknown,
    InvalidResponse,
    Rejected,
}
#[derive(Clone, Debug)]
pub struct GatewayError {
    pub kind: GatewayErrorKind,
    pub message: String,
}
impl GatewayError {
    pub fn new(kind: GatewayErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for GatewayError {}
