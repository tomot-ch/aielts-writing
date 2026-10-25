//! Real localhost HTTP acceptance using synthetic data, never real API credentials.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use writing_core::{
    infrastructure::{DocxExporter, FileWorkspace, SystemClock},
    model_gateway::OpenAiGateway,
    ports::SecretStore,
    *,
};

#[derive(Default)]
struct Secrets(Mutex<HashMap<String, String>>);
impl SecretStore for Secrets {
    fn set(&self, r: &str, key: &str) -> Result<()> {
        self.0.lock().unwrap().insert(r.into(), key.into());
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
fn service(w: &WritingService, secrets: Arc<Secrets>) -> Result<AiService> {
    AiService::with_ports(
        w.clone(),
        Arc::new(FileWorkspace),
        Arc::new(DocxExporter),
        Arc::new(SystemClock),
        secrets,
        Arc::new(OpenAiGateway::default()),
    )
}
async fn finish(ai: &AiService, job: &JobRef) -> Result<AnalysisRecord> {
    for _ in 0..200 {
        let state = ai.job(&job.id)?;
        if state.status == "completed" {
            return ai.analysis(&state.result_id.ok_or("missing result")?);
        }
        if state.status == "failed" {
            return Err(state.error.unwrap_or("failed".into()));
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Err("timeout".into())
}
fn activate(ai: &AiService, config: &ConfigRef) -> Result<()> {
    let prompt = ai.prompts()?.remove(0);
    ai.save_preferences(AiPreferences {
        schema_version: 1,
        active_slot: Some(1),
        presets: vec![AnalysisPreset {
            slot: 1,
            analysis_config_id: config.id.clone(),
            vision_config_id: None,
            prompt_id: prompt.id,
            record_issues: true,
            name: "上下文验收".into(),
        }],
    })?;
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("pass a new synthetic output directory")?,
    );
    let endpoint = std::env::args()
        .nth(2)
        .unwrap_or("http://127.0.0.1:18765/v1".into());
    if root.exists() {
        return Err("output directory must be new".into());
    }
    let w = WritingService::new(root.clone())?;
    let secrets = Arc::new(Secrets::default());
    let ai = service(&w, secrets.clone())?;
    let draft = ConfigDraft {
        id: None,
        name: "Responses 合成验收".into(),
        base_url: endpoint.clone(),
        api_key: "test-only-key".into(),
        model_name: "fixture-model".into(),
        context_limit: 32000,
        api_protocol: Some("responses".into()),
        context_policy: Some("auto".into()),
    };
    assert!(ai.test(draft.clone(), false).await?.ok);
    assert!(ai.test(draft.clone(), true).await?.ok);
    assert_eq!(ai.test_context(draft.clone()).await?.status, "supported");
    let config = ai.save_config(draft)?;
    activate(&ai, &config)?;
    let mut records = vec![];
    for marker in ["SYNTHETIC_ALPHA", "SYNTHETIC_BETA"] {
        let s = w.create_quick(Some(marker.into()))?;
        w.save(&s.id, 0, vec![TextRun { text: format!("{marker}. The chart shows a clear increase. The second category remains stable."), color: "black".into() }])?;
        w.submit_original(&s.id)?;
        let record = finish(
            &ai,
            &ai.start_conversation(&s.id, "Analyze this essay.".into())?,
        )
        .await?;
        assert!(record.context_state.is_some());
        records.push(record);
    }
    let a = &records[0];
    let b = &records[1];
    let first = ai
        .chat_turn(
            &a.id,
            "Explain the previous feedback.".into(),
            "turn-http-one".into(),
            false,
        )
        .await?;
    assert_eq!(first.context_mode, "remote_continue");
    assert!(first.reply.content.contains("ALPHA"));
    let reopened = service(&w, secrets)?;
    let next = reopened
        .chat_turn(
            &a.id,
            "Give one exercise.".into(),
            "turn-http-two".into(),
            false,
        )
        .await?;
    assert_eq!(next.context_mode, "remote_continue");
    let second = reopened
        .chat_turn(
            &b.id,
            "Explain this essay's weakness.".into(),
            "turn-http-three".into(),
            false,
        )
        .await?;
    assert!(second.reply.content.contains("BETA"));
    let local_draft = ConfigDraft {
        id: None,
        name: "普通接口合成验收".into(),
        base_url: endpoint,
        api_key: "test-only-key".into(),
        model_name: "fixture-model".into(),
        context_limit: 32000,
        api_protocol: None,
        context_policy: None,
    };
    reopened.test(local_draft.clone(), false).await?;
    reopened.test(local_draft.clone(), true).await?;
    let local = reopened.save_config(local_draft)?;
    activate(&reopened, &local)?;
    let manual = reopened
        .chat_turn(
            &b.id,
            "Use local history now.".into(),
            "turn-http-local".into(),
            false,
        )
        .await?;
    assert_eq!(manual.context_mode, "manual_history");
    assert!(manual.reply.content.contains("BETA"));
    let exports = root.join("exports");
    reopened.export(&a.id, "md", &exports)?;
    reopened.export(&a.id, "docx", &exports)?;
    assert_eq!(reopened.analysis(&a.id)?.messages.len(), 4);
    assert_eq!(reopened.analysis(&b.id)?.messages.len(), 4);
    println!(
        "{}",
        serde_json::json!({"ok":true,"root":root,"remoteIncremental":true,"restart":true,"essayIsolation":true,"localHistory":true,"exports":["md","docx"]})
    );
    Ok(())
}
