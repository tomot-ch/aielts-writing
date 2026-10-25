//! Durable chat turns. The transcript and remote cursor share one publication point.
use super::{review_system_prompt, AiService, Control};
use crate::{context::*, domain::id, *};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct InitialContext {
    pub response_id: String,
    pub connection_fingerprint: String,
    pub source_messages: Vec<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct PendingTurn {
    analysis_id: String,
    client_turn_id: String,
    message: String,
    expected_revision: u64,
    status: String,
    record: Option<AnalysisRecord>,
    receipt: Option<ChatTurnReceipt>,
    generation: Option<GenerationResult>,
}
struct Activity {
    control: Arc<Mutex<Control>>,
    session_id: String,
}
impl Drop for Activity {
    fn drop(&mut self) {
        if let Ok(mut ctrl) = self.control.lock() {
            if let Some(n) = ctrl.active_chats.get_mut(&self.session_id) {
                *n -= 1;
                if *n == 0 {
                    ctrl.active_chats.remove(&self.session_id);
                }
            }
        }
    }
}
impl AiService {
    fn turn_key(r: &AnalysisRecord, turn_id: &str) -> String {
        format!("{}-{turn_id}", r.id)
    }
    fn turn_path(&self, r: &AnalysisRecord, turn_id: &str) -> Result<PathBuf> {
        Self::check_id(turn_id)?;
        let (dir, _, _, _) = self.writing.analysis_source(&r.sources[0])?;
        Ok(dir
            .join("pending-turns")
            .join(format!("{}.json", Self::turn_key(r, turn_id))))
    }
    fn clear_turn(&self, r: &AnalysisRecord, turn_id: &str) {
        if let Ok(path) = self.turn_path(r, turn_id) {
            if self.store.exists(&path) {
                let _ = self.store.remove_file(&path);
            }
        }
        if let Ok(mut ctrl) = self.control.lock() {
            ctrl.received_turns.remove(&Self::turn_key(r, turn_id));
        }
    }
    fn find_pending(&self, r: &AnalysisRecord) -> Result<Option<PendingTurn>> {
        let memory = self
            .control
            .lock()
            .map_err(|_| "状态锁失败")?
            .received_turns
            .values()
            .find(|p| p.analysis_id == r.id)
            .cloned();
        if memory.is_some() {
            return Ok(memory);
        }
        let (dir, _, _, _) = self.writing.analysis_source(&r.sources[0])?;
        let folder = dir.join("pending-turns");
        if !self.store.exists(&folder) {
            return Ok(None);
        }
        let prefix = format!("{}-", r.id);
        for file in self.store.children(&folder)? {
            if file.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            if !file
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
            {
                continue;
            }
            let pending: PendingTurn = self.read(&file)?;
            if pending.analysis_id != r.id {
                continue;
            }
            // A crash may happen after publication but before journal cleanup.
            if let Some(receipt) = &pending.receipt {
                if r.messages.iter().any(|m| m.id == receipt.reply.id) {
                    self.clear_turn(r, &pending.client_turn_id);
                    continue;
                }
            }
            return Ok(Some(pending));
        }
        Ok(None)
    }
    pub fn pending_chat_turn(&self, analysis_id: &str) -> Result<Option<PendingChatTurn>> {
        let r = self.analysis(analysis_id)?;
        Ok(self.find_pending(&r)?.map(|p| PendingChatTurn {
            client_turn_id: p.client_turn_id,
            message: p.message,
            status: if p.record.is_some() {
                "received"
            } else {
                "outcome_unknown"
            }
            .into(),
        }))
    }
    fn commit_turn(&self, r: &AnalysisRecord, pending: &PendingTurn) -> Result<ChatTurnReceipt> {
        let record = pending.record.as_ref().ok_or("缺少待保存回复")?;
        let receipt = pending.receipt.as_ref().ok_or("缺少待保存回执")?;
        let latest = self.analysis(&r.id)?;
        if !latest.messages.iter().any(|m| m.id == receipt.reply.id) {
            if latest.revision != pending.expected_revision {
                return Err("对话已发生变化，待保存回复不能覆盖新的聊天记录".into());
            }
            self.persist_record(record)?;
        }
        self.clear_turn(r, &pending.client_turn_id);
        Ok(receipt.clone())
    }
    fn source_path(&self, r: &AnalysisRecord, file: &str) -> Result<PathBuf> {
        let name = file.strip_prefix("context/").ok_or("来源缓存路径无效")?;
        if name.is_empty()
            || name.contains("..")
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.'))
        {
            return Err("来源缓存路径无效".into());
        }
        let (dir, _, _, _) = self.writing.analysis_source(&r.sources[0])?;
        Ok(dir.join("context").join(name))
    }
    async fn chat_source(
        &self,
        r: &AnalysisRecord,
        c: &ConfigRef,
        vision: Option<&ConfigRef>,
        prompt: &PromptRef,
        fp: &str,
    ) -> Result<(Vec<Value>, String, usize)> {
        let (task_type, _) = self.source_metadata(&r.sources[0])?;
        let system_prompt = review_system_prompt(task_type.as_deref(), &prompt.content);
        let mut cached = None;
        if let Some(state) = &r.context_state {
            if let Ok(path) = self.source_path(r, &state.source_file) {
                if self.store.exists(&path) {
                    let source: Vec<Value> = self.read(&path)?;
                    let has_image = source.iter().any(|m| {
                        m["content"]
                            .as_array()
                            .is_some_and(|ps| ps.iter().any(|p| p["type"] == "image_url"))
                    });
                    if c.vision_capable || !has_image {
                        cached = Some(source);
                    }
                }
            }
        }
        let mut source = if let Some(source) = cached {
            source
        } else {
            self.source_messages(&r.sources[0], c, vision, &system_prompt, false)
                .await?
        };
        // The top-level instructions are current-turn configuration, never an old snapshot.
        source.retain(|m| !matches!(m["role"].as_str(), Some("system" | "developer")));
        let size = source.iter().map(text_size).sum();
        source.insert(
            0,
            json!({"role":"system","content":system_prompt}),
        );
        let file = format!("context/{}-{}.source.json", r.id, &fp[..16]);
        self.save(&self.source_path(r, &file)?, &source)?;
        Ok((source, file, size))
    }
    pub async fn chat_turn(
        &self,
        analysis_id: &str,
        message: String,
        client_turn_id: String,
        retry_uncertain: bool,
    ) -> Result<ChatTurnReceipt> {
        Self::check_id(&client_turn_id)?;
        if client_turn_id.len() > 120 || message.trim().is_empty() {
            return Err("请输入聊天内容，并使用有效的请求标识".into());
        }
        let before = self.analysis(analysis_id)?;
        if before.many {
            return Err("请在单篇分析中继续聊天".into());
        }
        let sid = before.sources[0].session_id.clone();
        let lock = {
            let mut ctrl = self.control.lock().map_err(|_| "状态锁失败")?;
            *ctrl.active_chats.entry(sid.clone()).or_default() += 1;
            ctrl.chat_locks
                .entry(analysis_id.into())
                .or_default()
                .clone()
        };
        let _activity = Activity {
            control: self.control.clone(),
            session_id: sid,
        };
        let _guard = lock.lock().await;
        let r = self.analysis(analysis_id)?;
        if let Some(pair) = r.messages.windows(2).find(|pair| {
            pair[0].role == "user" && pair[0].client_turn_id.as_deref() == Some(&client_turn_id)
        }) {
            if pair[0].content == message {
                return Ok(ChatTurnReceipt {
                    client_turn_id,
                    user: pair[0].clone(),
                    reply: pair[1].clone(),
                    revision: r.revision,
                    context_mode: if r.context_state.is_some() {
                        "remote_continue"
                    } else {
                        "manual_history"
                    }
                    .into(),
                    notice: Some("本轮已保存，已恢复原回复。".into()),
                });
            }
            return Err("该请求标识已用于另一条消息".into());
        }
        let existing = self.find_pending(&r)?;
        let mut force_bootstrap = false;
        if let Some(pending) = existing {
            if pending.client_turn_id != client_turn_id || pending.message != message {
                return Err("此对话有尚未处理的请求，请先恢复待保存回复或重新发送原问题。".into());
            }
            if pending.record.is_some() {
                return self.commit_turn(&r, &pending);
            }
            if !retry_uncertain {
                return Err(
                    "上次模型结果未确认，请使用“重新发送原问题”；此操作可能再次计费。".into(),
                );
            }
            force_bootstrap = true;
        }
        let preset = self.active_preset()?;
        let c = self.config(&preset.analysis_config_id)?;
        let key = self.key_for(&c)?;
        let vision = preset
            .vision_config_id
            .as_ref()
            .map(|id| self.config(id))
            .transpose()?;
        let prompt = self
            .prompts()?
            .into_iter()
            .find(|p| p.id == preset.prompt_id)
            .ok_or("提示词不存在")?;
        let connection_fp = fingerprint(&c, &key);
        let disabled = self
            .control
            .lock()
            .map_err(|_| "状态锁失败")?
            .disabled_context
            .contains(&connection_fp);
        let remote = remote_enabled(&c, &key)? && !disabled;
        if disabled && policy(&c) == "remote_required" {
            return Err("服务端上下文能力已失效，请重新测试此连接".into());
        }
        let fp = binding(&c, &key, &prompt.version_id, &r);
        let state = r.context_state.as_ref();
        let continuing =
            remote && !force_bootstrap && state.is_some_and(|s| can_continue(s, &fp, &r));
        let (task_type, _) = self.source_metadata(&r.sources[0])?;
        let instructions = json!({"role":"system","content":review_system_prompt(task_type.as_deref(), &prompt.content)});
        let history_size = history(&r).iter().map(text_size).sum::<usize>();
        let (mut messages, mut source_file, mut source_size) = if continuing {
            let state = state.unwrap();
            (
                vec![instructions.clone()],
                state.source_file.clone(),
                state.source_characters,
            )
        } else {
            self.chat_source(&r, &c, vision.as_ref(), &prompt, &fp)
                .await?
        };
        let logical_size =
            source_size + text_size(&instructions) + history_size + message.chars().count();
        if logical_size > c.context_limit {
            return Err("聊天上下文超过配置预算，请提高预算或新建对话；未截断历史。云端接续同样受上下文窗口限制。".into());
        }
        if !continuing {
            messages.extend(history(&r));
        }
        messages.push(json!({"role":"user","content":message}));
        let request = GenerationRequest {
            messages,
            previous_response_id: if continuing {
                state.map(|s| s.response_id.clone())
            } else {
                None
            },
            store: remote,
        };
        let mut pending = PendingTurn {
            analysis_id: r.id.clone(),
            client_turn_id: client_turn_id.clone(),
            message: message.clone(),
            expected_revision: r.revision,
            status: "sending".into(),
            record: None,
            receipt: None,
            generation: None,
        };
        let pending_path = self.turn_path(&r, &client_turn_id)?;
        self.save(&pending_path, &pending)?;
        let mut mode = if continuing {
            "remote_continue"
        } else if remote {
            "remote_bootstrap"
        } else {
            "manual_history"
        };
        let mut notice = if force_bootstrap {
            Some("已从本地已保存历史重新建立上下文。".into())
        } else {
            None
        };
        let mut result = self.gateway.generate(&c, &key, request).await;
        let recover_missing = result
            .as_ref()
            .is_err_and(|e| e.kind == GatewayErrorKind::RemoteStateMissing)
            && continuing;
        let fallback_local = result
            .as_ref()
            .is_err_and(|e| e.kind == GatewayErrorKind::UnsupportedContinuation)
            && remote;
        if policy(&c) == "auto" && (recover_missing || fallback_local) {
            let (mut full, file, size) = match self
                .chat_source(&r, &c, vision.as_ref(), &prompt, &fp)
                .await
            {
                Ok(source) => source,
                Err(e) => {
                    // The provider explicitly rejected the first request and the
                    // recovery request has not been sent. Its outcome is known.
                    self.clear_turn(&r, &client_turn_id);
                    return Err(e);
                }
            };
            source_file = file;
            source_size = size;
            full.extend(history(&r));
            full.push(json!({"role":"user","content":message}));
            if full.iter().map(text_size).sum::<usize>() > c.context_limit {
                self.clear_turn(&r, &client_turn_id);
                return Err(
                    "恢复后的完整上下文超过预算；未截断历史，请提高预算或新建对话。".into(),
                );
            }
            if fallback_local {
                self.control
                    .lock()
                    .map_err(|_| "状态锁失败")?
                    .disabled_context
                    .insert(connection_fp);
                mode = "manual_history";
                notice =
                    Some("接口已明确拒绝上下文接续，本次改用本地完整历史；可重新测试连接。".into());
            } else {
                mode = "remote_bootstrap";
                notice = Some("服务端上下文已失效，已从本地历史恢复。".into());
            }
            result = self
                .gateway
                .generate(
                    &c,
                    &key,
                    GenerationRequest {
                        messages: full,
                        previous_response_id: None,
                        store: !fallback_local,
                    },
                )
                .await;
        }
        let generation = match result {
            Ok(generation) => generation,
            Err(e) => {
                if e.kind == GatewayErrorKind::OutcomeUnknown {
                    pending.status = "outcome_unknown".into();
                    let _ = self.save(&pending_path, &pending);
                } else {
                    self.clear_turn(&r, &client_turn_id);
                }
                return Err(e.to_string());
            }
        };
        let user = ChatMessage {
            schema_version: 1,
            id: id("message"),
            analysis_id: r.id.clone(),
            role: "user".into(),
            content: message,
            created_at: self.clock.now(),
            config_id: Some(c.id.clone()),
            model_name: Some(c.model_name.clone()),
            prompt_version_id: Some(prompt.version_id.clone()),
            client_turn_id: Some(client_turn_id.clone()),
        };
        let reply = ChatMessage {
            id: id("message"),
            role: "assistant".into(),
            content: generation.text.clone(),
            ..user.clone()
        };
        let mut record = r.clone();
        record.messages.extend([user.clone(), reply.clone()]);
        record.revision += 1;
        record.last_turn_id = Some(client_turn_id.clone());
        record.context_state = if mode != "manual_history" {
            let Some(response_id) = generation.response_id.clone() else {
                return Err(
                    "模型已返回回复但缺少服务端标识，本次结果未确认，请重新测试上下文能力。".into(),
                );
            };
            Some(ConversationContext {
                response_id,
                binding_fingerprint: fp,
                history_digest: history_digest(&record),
                epoch: state.map_or(1, |s| {
                    s.epoch + u64::from(!continuing || mode == "remote_bootstrap")
                }),
                source_file,
                source_characters: source_size,
                logical_characters: logical_size + reply.content.chars().count(),
            })
        } else {
            None
        };
        let receipt = ChatTurnReceipt {
            client_turn_id,
            user,
            reply,
            revision: record.revision,
            context_mode: mode.into(),
            notice,
        };
        pending.status = "received".into();
        pending.record = Some(record);
        pending.receipt = Some(receipt);
        pending.generation = Some(generation);
        // Keep the received result in memory even if writing the recovery checkpoint fails.
        self.control
            .lock()
            .map_err(|_| "状态锁失败")?
            .received_turns
            .insert(Self::turn_key(&r, &pending.client_turn_id), pending.clone());
        self.save(&pending_path, &pending).map_err(|_| {
            "回复已收到，但本地保存失败。请点击恢复已收到的回复；不要重新发起模型请求。".to_string()
        })?;
        self.commit_turn(&r, &pending)
            .map_err(|e| format!("回复已收到，但归档未完成：{e}。请恢复已收到的回复。"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        infrastructure::{DocxExporter, SystemClock},
        ports::*,
    };
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicBool, Ordering},
    };

    #[derive(Default)]
    struct Secrets(Mutex<HashMap<String, String>>);
    impl SecretStore for Secrets {
        fn set(&self, r: &str, s: &str) -> Result<()> {
            self.0.lock().unwrap().insert(r.into(), s.into());
            Ok(())
        }
        fn get(&self, r: &str) -> Result<String> {
            self.0
                .lock()
                .unwrap()
                .get(r)
                .cloned()
                .ok_or("missing test secret".into())
        }
    }
    #[derive(Default)]
    struct GatewayState {
        seen: Vec<GenerationRequest>,
        responses: HashMap<String, Vec<Value>>,
        error: Option<GatewayErrorKind>,
    }
    #[derive(Default)]
    struct StatefulGateway {
        inner: Mutex<GatewayState>,
        ignore_id: AtomicBool,
    }
    #[async_trait::async_trait]
    impl ModelGateway for StatefulGateway {
        async fn complete(&self, c: &ConfigRef, key: &str, m: Vec<Value>) -> Result<String> {
            self.generate(c, key, GenerationRequest::stateless(m))
                .await
                .map(|r| r.text)
                .map_err(|e| e.to_string())
        }
        async fn generate(
            &self,
            _: &ConfigRef,
            _: &str,
            request: GenerationRequest,
        ) -> std::result::Result<GenerationResult, GatewayError> {
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            let mut state = self.inner.lock().unwrap();
            state.seen.push(request.clone());
            if let Some(error) = state.error.take() {
                return Err(GatewayError::new(error, "injected provider failure"));
            }
            let mut full = if let Some(previous) = &request.previous_response_id {
                if self.ignore_id.load(Ordering::SeqCst) {
                    vec![]
                } else {
                    state.responses.get(previous).cloned().ok_or_else(|| {
                        GatewayError::new(GatewayErrorKind::RemoteStateMissing, "expired")
                    })?
                }
            } else {
                vec![]
            };
            full.extend(request.messages.clone());
            let raw = serde_json::to_string(&full).unwrap();
            let query = request.messages.last().unwrap()["content"]
                .as_str()
                .unwrap_or("");
            let text = if query.contains("Return only the exact secret") {
                raw.split("Remember this secret token for the next turn: ")
                    .nth(1)
                    .and_then(|s| s.split('.').next())
                    .unwrap_or("NO_MEMORY")
                    .to_string()
            } else if raw.contains("WRITING_OK") {
                "WRITING_OK".into()
            } else if raw.contains("four digits") {
                "2468".into()
            } else if query.contains("Remember this secret") {
                "OK".into()
            } else if raw.contains("SYNTHETIC_ALPHA") && raw.contains("SYNTHETIC_BETA") {
                panic!("essay contexts mixed")
            } else if raw.contains("SYNTHETIC_ALPHA") {
                "ALPHA feedback\n## 问题归纳\nDevelop your evidence.".into()
            } else {
                "BETA feedback\n## 问题归纳\nImprove comparisons.".into()
            };
            let response_id = id("response");
            full.push(json!({"role":"assistant","content":text}));
            if request.store {
                state.responses.insert(response_id.clone(), full);
            }
            Ok(GenerationResult {
                text,
                response_id: request.store.then_some(response_id),
                output_items: vec![],
                usage: None,
                provider_request_id: None,
            })
        }
    }
    struct Fixture {
        root: PathBuf,
        writing: WritingService,
        ai: AiService,
        gateway: Arc<StatefulGateway>,
        secrets: Arc<Secrets>,
        store: Arc<crate::test_support::FailOnceStore>,
        config: ConfigRef,
    }
    impl Fixture {
        async fn new(remote: bool, ignore_id: bool) -> Self {
            let root = std::env::temp_dir().join(id("dual-context-test"));
            let writing = WritingService::new(root.clone()).unwrap();
            let store = Arc::new(crate::test_support::FailOnceStore::default());
            let gateway = Arc::new(StatefulGateway::default());
            gateway.ignore_id.store(ignore_id, Ordering::SeqCst);
            let secrets = Arc::new(Secrets::default());
            let ai = AiService::with_ports(
                writing.clone(),
                store.clone(),
                Arc::new(DocxExporter),
                Arc::new(SystemClock),
                secrets.clone(),
                gateway.clone(),
            )
            .unwrap();
            let draft = ConfigDraft {
                id: None,
                name: "fixture".into(),
                base_url: "http://127.0.0.1:1/v1".into(),
                api_key: "synthetic-only-key".into(),
                model_name: "synthetic".into(),
                context_limit: 32000,
                api_protocol: remote.then(|| "responses".into()),
                context_policy: remote.then(|| "auto".into()),
            };
            assert!(ai.test(draft.clone(), false).await.unwrap().ok);
            assert!(ai.test(draft.clone(), true).await.unwrap().ok);
            if remote {
                ai.test_context(draft.clone()).await.unwrap();
            }
            let config = ai.save_config(draft).unwrap();
            Self {
                root,
                writing,
                ai,
                gateway,
                secrets,
                store,
                config,
            }
        }
        async fn essay(&self, text: &str) -> AnalysisRecord {
            let s = self.writing.create_quick(None).unwrap();
            self.writing
                .save(
                    &s.id,
                    0,
                    vec![TextRun {
                        text: text.into(),
                        color: "black".into(),
                    }],
                )
                .unwrap();
            self.writing.submit_original(&s.id).unwrap();
            let job = self
                .ai
                .start_conversation(&s.id, "Please analyze this essay.".into())
                .unwrap();
            for _ in 0..200 {
                let state = self.ai.job(&job.id).unwrap();
                if state.status == "completed" {
                    return self.ai.analysis(&state.result_id.unwrap()).unwrap();
                }
                if state.status == "failed" {
                    panic!("first reply failed: {:?}", state.error);
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            panic!("timed out")
        }
        fn count(&self) -> usize {
            self.gateway.inner.lock().unwrap().seen.len()
        }
        fn last(&self) -> GenerationRequest {
            self.gateway
                .inner
                .lock()
                .unwrap()
                .seen
                .last()
                .unwrap()
                .clone()
        }
        fn reopen(&self) -> AiService {
            AiService::with_ports(
                self.writing.clone(),
                self.store.clone(),
                Arc::new(DocxExporter),
                Arc::new(SystemClock),
                self.secrets.clone(),
                self.gateway.clone(),
            )
            .unwrap()
        }
        async fn edit_config(&self, model: &str, policy_value: &str) {
            let draft = ConfigDraft {
                id: Some(self.config.id.clone()),
                name: "fixture".into(),
                base_url: self.config.base_url.clone(),
                api_key: "".into(),
                model_name: model.into(),
                context_limit: 32000,
                api_protocol: Some("responses".into()),
                context_policy: Some(policy_value.into()),
            };
            self.ai.test(draft.clone(), false).await.unwrap();
            self.ai.test(draft.clone(), true).await.unwrap();
            self.ai.test_context(draft.clone()).await.unwrap();
            self.ai.save_config(draft).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn remote_turns_are_incremental_and_resume_after_restart_with_essay_isolation() {
        let f = Fixture::new(true, false).await;
        let a = f.essay("SYNTHETIC_ALPHA original").await;
        let original = a.context_state.as_ref().unwrap().response_id.clone();
        let first =
            f.ai.chat_turn(&a.id, "first question".into(), "turn-one".into(), false)
                .await
                .unwrap();
        assert_eq!(first.context_mode, "remote_continue");
        let wire = f.last();
        assert_eq!(
            wire.previous_response_id.as_deref(),
            Some(original.as_str())
        );
        assert_eq!(wire.messages.len(), 2); // instructions + only the new question
        assert!(!serde_json::to_string(&wire.messages)
            .unwrap()
            .contains("SYNTHETIC_ALPHA"));
        let saved = f.ai.analysis(&a.id).unwrap();
        let next_id = saved.context_state.unwrap().response_id;
        assert_ne!(next_id, original);
        let reopened = f.reopen();
        reopened
            .chat_turn(&a.id, "second question".into(), "turn-two".into(), false)
            .await
            .unwrap();
        assert_eq!(
            f.last().previous_response_id.as_deref(),
            Some(next_id.as_str())
        );
        let b = f.essay("SYNTHETIC_BETA original").await;
        let response =
            f.ai.chat_turn(&b.id, "another question".into(), "turn-three".into(), false)
                .await
                .unwrap();
        assert!(response.reply.content.starts_with("BETA"));
        assert_ne!(f.last().previous_response_id, Some(original));
        assert_eq!(reopened.analysis(&a.id).unwrap().messages.len(), 4);
    }
    #[tokio::test]
    async fn silently_ignored_ids_do_not_pass_capability_test_and_full_history_is_sent() {
        let f = Fixture::new(true, true).await;
        assert_eq!(
            f.config.context_capability.as_ref().unwrap().status,
            "unverified"
        );
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        assert!(r.context_state.is_none());
        let result =
            f.ai.chat_turn(&r.id, "new question".into(), "turn-local".into(), false)
                .await
                .unwrap();
        assert_eq!(result.context_mode, "manual_history");
        let request = f.last();
        assert!(!request.store && request.previous_response_id.is_none());
        assert_eq!(
            request
                .messages
                .iter()
                .filter(|m| m["content"] == "Please analyze this essay.")
                .count(),
            1
        );
        assert_eq!(
            request
                .messages
                .iter()
                .filter(|m| m["content"] == "new question")
                .count(),
            1
        );
        assert!(serde_json::to_string(&request.messages)
            .unwrap()
            .contains("SYNTHETIC_ALPHA"));
    }
    #[tokio::test]
    async fn legacy_chat_config_keeps_full_history_and_old_dtos_load() {
        let f = Fixture::new(false, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        let result =
            f.ai.chat_turn(&r.id, "follow up".into(), "turn-legacy".into(), false)
                .await
                .unwrap();
        assert_eq!(result.context_mode, "manual_history");
        assert_eq!(f.last().messages.len(), 5);
        let mut old = serde_json::to_value(&r).unwrap();
        for field in ["contextState", "revision", "lastTurnId"] {
            old.as_object_mut().unwrap().remove(field);
        }
        let legacy: AnalysisRecord = serde_json::from_value(old).unwrap();
        assert!(legacy.context_state.is_none());
        assert_eq!(legacy.revision, 0);
        let mut old = serde_json::to_value(&f.config).unwrap();
        for field in ["apiProtocol", "contextPolicy", "contextCapability"] {
            old.as_object_mut().unwrap().remove(field);
        }
        let legacy: ConfigRef = serde_json::from_value(old).unwrap();
        assert_eq!(protocol(&legacy), "chat_completions");
        assert_eq!(policy(&legacy), "local");
    }
    #[tokio::test]
    async fn known_expiration_bootstraps_once_without_duplicating_the_current_question() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.gateway.inner.lock().unwrap().responses.clear();
        let before = f.count();
        let reply =
            f.ai.chat_turn(
                &r.id,
                "restore question".into(),
                "turn-expired".into(),
                false,
            )
            .await
            .unwrap();
        assert_eq!(f.count(), before + 2);
        assert_eq!(reply.context_mode, "remote_bootstrap");
        assert!(f.last().previous_response_id.is_none());
        assert_eq!(
            f.last()
                .messages
                .iter()
                .filter(|m| m["content"] == "restore question")
                .count(),
            1
        );
        assert_eq!(
            f.ai.analysis(&r.id).unwrap().context_state.unwrap().epoch,
            2
        );
    }
    #[tokio::test]
    async fn configuration_changes_bootstrap_and_local_policy_can_take_over() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.ai.chat_turn(&r.id, "old question".into(), "turn-old-model".into(), false)
            .await
            .unwrap();
        f.edit_config("new-model", "auto").await;
        let reply =
            f.ai.chat_turn(
                &r.id,
                "new model question".into(),
                "turn-new-model".into(),
                false,
            )
            .await
            .unwrap();
        assert_eq!(reply.context_mode, "remote_bootstrap");
        assert!(f.last().previous_response_id.is_none());
        assert!(f
            .last()
            .messages
            .iter()
            .any(|m| m["content"] == "old question"));
        f.edit_config("new-model", "local").await;
        let reply =
            f.ai.chat_turn(&r.id, "local now".into(), "turn-local-policy".into(), false)
                .await
                .unwrap();
        assert_eq!(reply.context_mode, "manual_history");
        assert!(!f.last().store);
        assert!(f.ai.analysis(&r.id).unwrap().context_state.is_none());
    }
    #[tokio::test]
    async fn timeout_is_not_retried_and_explicit_retry_uses_a_new_remote_branch() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.gateway.inner.lock().unwrap().error = Some(GatewayErrorKind::OutcomeUnknown);
        let before = f.count();
        assert!(f
            .ai
            .chat_turn(&r.id, "uncertain".into(), "turn-unknown".into(), false)
            .await
            .is_err());
        assert_eq!(f.count(), before + 1);
        assert_eq!(
            f.ai.pending_chat_turn(&r.id).unwrap().unwrap().status,
            "outcome_unknown"
        );
        assert!(f
            .ai
            .chat_turn(&r.id, "different".into(), "turn-other".into(), false)
            .await
            .is_err());
        assert!(f
            .ai
            .chat_turn(&r.id, "uncertain".into(), "turn-unknown".into(), false)
            .await
            .is_err());
        assert_eq!(f.count(), before + 1);
        let receipt = f
            .reopen()
            .chat_turn(&r.id, "uncertain".into(), "turn-unknown".into(), true)
            .await
            .unwrap();
        assert_eq!(receipt.context_mode, "remote_bootstrap");
        assert!(f.last().previous_response_id.is_none());
        assert_eq!(f.ai.analysis(&r.id).unwrap().messages.len(), 2);
    }
    #[tokio::test]
    async fn failed_publication_recovers_after_restart_without_another_model_call() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.store.fail_next(&format!("analysis/{}.json", r.id));
        let before = f.count();
        assert!(f
            .ai
            .chat_turn(&r.id, "save this".into(), "turn-disk".into(), false)
            .await
            .is_err());
        assert_eq!(f.count(), before + 1);
        assert!(f.ai.analysis(&r.id).unwrap().messages.is_empty());
        let ai = f.reopen();
        assert_eq!(
            ai.pending_chat_turn(&r.id).unwrap().unwrap().status,
            "received"
        );
        ai.chat_turn(&r.id, "save this".into(), "turn-disk".into(), false)
            .await
            .unwrap();
        assert_eq!(f.count(), before + 1);
        assert_eq!(ai.analysis(&r.id).unwrap().messages.len(), 2);
        assert!(ai.pending_chat_turn(&r.id).unwrap().is_none());
    }
    #[tokio::test]
    async fn failed_received_checkpoint_keeps_result_in_memory_for_save_only_retry() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.store.fail_when(
            &format!("{}-turn-checkpoint.json", r.id),
            "\"status\": \"received\"",
        );
        let before = f.count();
        assert!(f
            .ai
            .chat_turn(&r.id, "save once".into(), "turn-checkpoint".into(), false)
            .await
            .is_err());
        assert_eq!(
            f.ai.pending_chat_turn(&r.id).unwrap().unwrap().status,
            "received"
        );
        f.ai.chat_turn(&r.id, "save once".into(), "turn-checkpoint".into(), false)
            .await
            .unwrap();
        assert_eq!(f.count(), before + 1);
    }
    #[tokio::test]
    async fn concurrent_turns_serialize_and_old_client_ids_cannot_duplicate_messages() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        let before = f.count();
        let (a, b) = tokio::join!(
            f.ai.chat_turn(&r.id, "one".into(), "turn-concurrent-one".into(), false),
            f.ai.chat_turn(&r.id, "two".into(), "turn-concurrent-two".into(), false)
        );
        a.unwrap();
        b.unwrap();
        let saved = f.ai.analysis(&r.id).unwrap();
        assert_eq!(saved.messages.len(), 4);
        let seen = f.gateway.inner.lock().unwrap().seen.clone();
        assert_ne!(
            seen[before].previous_response_id,
            seen[before + 1].previous_response_id
        );
        f.ai.chat_turn(&r.id, "one".into(), "turn-concurrent-one".into(), false)
            .await
            .unwrap();
        assert_eq!(f.count(), before + 2);
        assert_eq!(f.ai.analysis(&r.id).unwrap().messages.len(), 4);
    }
    #[tokio::test]
    async fn auth_failures_never_fall_back_and_explicit_parameter_rejection_can() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.gateway.inner.lock().unwrap().error = Some(GatewayErrorKind::Unauthorized);
        let before = f.count();
        assert!(f
            .ai
            .chat_turn(&r.id, "auth".into(), "turn-auth".into(), false)
            .await
            .is_err());
        assert_eq!(f.count(), before + 1);
        assert!(f.ai.pending_chat_turn(&r.id).unwrap().is_none());
        f.gateway.inner.lock().unwrap().error = Some(GatewayErrorKind::UnsupportedContinuation);
        let receipt =
            f.ai.chat_turn(&r.id, "fallback".into(), "turn-fallback".into(), false)
                .await
                .unwrap();
        assert_eq!(receipt.context_mode, "manual_history");
        assert!(!f.last().store);
        let receipt =
            f.ai.chat_turn(&r.id, "stay local".into(), "turn-stay-local".into(), false)
                .await
                .unwrap();
        assert_eq!(receipt.context_mode, "manual_history");
        let next_conversation = f.essay("SYNTHETIC_BETA original").await;
        assert!(next_conversation.context_state.is_none());
        assert!(!f.last().store);
    }
    #[tokio::test]
    async fn remote_mode_still_checks_the_logical_history_budget() {
        let f = Fixture::new(true, false).await;
        let r = f
            .essay(&format!("SYNTHETIC_ALPHA {}", "evidence ".repeat(300)))
            .await;
        f.ai.save_config(ConfigDraft {
            id: Some(f.config.id.clone()),
            name: "fixture".into(),
            base_url: f.config.base_url.clone(),
            api_key: "".into(),
            model_name: f.config.model_name.clone(),
            context_limit: 1024,
            api_protocol: Some("responses".into()),
            context_policy: Some("auto".into()),
        })
        .unwrap();
        let before = f.count();
        assert!(f
            .ai
            .chat_turn(&r.id, "too long".into(), "turn-budget".into(), false)
            .await
            .unwrap_err()
            .contains("预算"));
        assert_eq!(f.count(), before);
        assert!(f.ai.pending_chat_turn(&r.id).unwrap().is_none());
    }
    #[tokio::test]
    async fn a_prompt_change_bootstraps_with_new_instructions_and_preserves_history() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        let prompt =
            f.ai.save_prompt(PromptDraft {
                id: None,
                name: "cohesion".into(),
                content: "Focus on cohesion only.".into(),
            })
            .unwrap();
        let mut prefs = f.ai.preferences().unwrap();
        prefs.presets[0].prompt_id = prompt.id;
        f.ai.save_preferences(prefs).unwrap();
        let receipt =
            f.ai.chat_turn(&r.id, "new prompt".into(), "turn-prompt".into(), false)
                .await
                .unwrap();
        assert_eq!(receipt.context_mode, "remote_bootstrap");
        assert!(f.last().messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("Focus on cohesion only."));
        assert!(f
            .last()
            .messages
            .iter()
            .any(|m| m["content"] == "Please analyze this essay."));
    }
    #[tokio::test]
    async fn required_remote_policy_does_not_silently_recover_or_fall_back() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.edit_config("synthetic", "remote_required").await;
        f.gateway.inner.lock().unwrap().responses.clear();
        let before = f.count();
        assert!(f
            .ai
            .chat_turn(
                &r.id,
                "remote required".into(),
                "turn-required".into(),
                false
            )
            .await
            .is_err());
        assert_eq!(f.count(), before + 1);
        assert!(f.ai.analysis(&r.id).unwrap().messages.is_empty());
    }
    #[tokio::test]
    async fn rejected_remote_turn_with_failed_local_recovery_does_not_become_uncertain() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        f.gateway.inner.lock().unwrap().responses.clear();
        f.store.fail_next(".source.json");
        let before = f.count();
        assert!(f
            .ai
            .chat_turn(
                &r.id,
                "recover source".into(),
                "turn-source-failure".into(),
                false
            )
            .await
            .is_err());
        assert_eq!(f.count(), before + 1);
        assert!(f.ai.pending_chat_turn(&r.id).unwrap().is_none());
        f.ai.chat_turn(
            &r.id,
            "recover source".into(),
            "turn-source-retry".into(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(f.ai.analysis(&r.id).unwrap().messages.len(), 2);
    }
    #[tokio::test]
    async fn another_conversations_corrupt_journal_does_not_block_this_conversation() {
        let f = Fixture::new(true, false).await;
        let r = f.essay("SYNTHETIC_ALPHA original").await;
        let (dir, _, _, _) = f.writing.analysis_source(&r.sources[0]).unwrap();
        f.store
            .write_atomic(
                &dir.join("pending-turns/analysis-other-broken.json"),
                b"invalid json",
            )
            .unwrap();
        assert!(f.ai.pending_chat_turn(&r.id).unwrap().is_none());
        f.ai.chat_turn(
            &r.id,
            "isolated question".into(),
            "turn-isolated".into(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(f.ai.analysis(&r.id).unwrap().messages.len(), 2);
    }
}
