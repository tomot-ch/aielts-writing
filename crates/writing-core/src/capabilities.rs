use super::AiService;
use crate::{context::*, domain::id, ConfigDraft, ContextCapability, Result};
use serde_json::json;

impl AiService {
    /// Two genuinely separate calls: the second must recover a high-entropy secret
    /// using only the response ID. No user essay or client-side transcript is used.
    pub async fn test_context(&self, draft: ConfigDraft) -> Result<ContextCapability> {
        let (c, key, fp) = self.candidate(&draft)?;
        let tested_at = self.clock.now();
        let cap = if protocol(&c) != "responses" {
            capability(
                "unsupported",
                tested_at,
                fp.clone(),
                Some(
                    "普通聊天接口使用本地完整历史；可选择 Responses 接口后测试服务端接续。".into(),
                ),
            )
        } else {
            let secret = id("context");
            let mut probe_ids = vec![];
            let probe = async {
                let first = self.gateway.generate(&c, &key, GenerationRequest {
                    messages: vec![json!({"role":"user","content":format!("Remember this secret token for the next turn: {secret}. Reply only OK.")})],
                    previous_response_id: None, store: true,
                }).await?;
                let response_id = first.response_id.ok_or_else(|| GatewayError::new(GatewayErrorKind::UnsupportedContinuation, "接口未返回响应 ID"))?;
                probe_ids.push(response_id.clone());
                // Some providers need a brief propagation interval for stored responses.
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                let result = self.gateway.generate(&c, &key, GenerationRequest {
                    messages: vec![json!({"role":"user","content":"Return only the exact secret token from my previous turn, without any explanation."})],
                    previous_response_id: Some(response_id), store: true,
                }).await;
                if let Ok(reply) = &result { if let Some(id) = &reply.response_id { probe_ids.push(id.clone()); } }
                result
            }.await;
            for id in probe_ids {
                self.gateway.discard_probe_response(&c, &key, &id).await;
            }
            match probe {
                Ok(reply) if reply.text.trim() == secret => {
                    capability("supported", tested_at, fp.clone(), None)
                }
                Ok(_) => capability(
                    "unverified",
                    tested_at,
                    fp.clone(),
                    Some(
                        "第二轮未能准确复述随机信息，尚未确认服务端历史；自动模式使用本地管理。"
                            .into(),
                    ),
                ),
                Err(e) => capability(
                    if e.kind == GatewayErrorKind::UnsupportedContinuation {
                        "unsupported"
                    } else {
                        "unverified"
                    },
                    tested_at,
                    fp.clone(),
                    Some(e.to_string()),
                ),
            }
        };
        let mut ctrl = self.control.lock().map_err(|_| "状态锁失败")?;
        if cap.status == "supported" {
            ctrl.disabled_context.remove(&fp);
        }
        ctrl.tests.entry(fp).or_default().2 = Some(cap.clone());
        Ok(cap)
    }
}
