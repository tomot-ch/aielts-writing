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
            .ok_or("pass a NEW isolated test directory")?,
    );
    let endpoint = std::env::args()
        .nth(2)
        .unwrap_or("http://127.0.0.1:18877/v1".into());
    if root.exists() {
        return Err("test output directory must be new".into());
    }
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/annotated-review.json"
    ))
    .unwrap();
    let runs: Vec<TextRun> = serde_json::from_value(fixture["runs"].clone()).unwrap();
    let w = WritingService::new(root.clone())?;
    let secrets = Arc::new(Secrets::default());
    let mut ids = vec![];
    for protocol in ["chat_completions", "responses"] {
        let ai = service(&w, secrets.clone())?;
        let draft = ConfigDraft {
            id: None,
            name: format!("合成批注 {protocol}"),
            base_url: endpoint.clone(),
            api_key: "test-only-key".into(),
            model_name: "fixture-model".into(),
            context_limit: 32000,
            api_protocol: Some(protocol.into()),
            context_policy: Some("auto".into()),
        };
        assert!(ai.test(draft.clone(), false).await?.ok);
        assert!(ai.test(draft.clone(), true).await?.ok);
        if protocol == "responses" {
            assert_eq!(ai.test_context(draft.clone()).await?.status, "supported");
        }
        let config = ai.save_config(draft)?;
        activate(&ai, &config)?;
        let session = w.create_quick(Some(format!("批注合成验收 {protocol}")))?;
        w.save(&session.id, 0, runs.clone())?;
        let fixed = w.submit_original(&session.id)?;
        let record = finish(
            &ai,
            &ai.start_conversation(&session.id, "请批阅并生成批注".into())?,
        )
        .await?;
        let review = record.review.as_ref().ok_or("missing review")?;
        assert_eq!(review.runs, runs);
        assert_eq!(review.source.version_id, fixed.id);
        assert_eq!(review.annotations.len(), 10);
        assert!(review.annotations[0].start.is_some());
        assert!(review.annotations[5].start.is_none());
        assert!(review.annotations[6].start.is_some());
        assert!(review.annotations[7].start.is_none());
        assert_eq!(
            review
                .scores
                .iter()
                .find(|s| s.criterion == "overall")
                .unwrap()
                .band,
            None
        );
        assert_eq!(record.markdown, review::markdown(review));
        // Change the current editor/version, reopen the service, and continue the old review.
        w.begin_original_edit(&session.id)?;
        let view = w.open(&session.id)?;
        w.save(
            &session.id,
            view.save_revision,
            vec![TextRun {
                text: "A NEW FIXED VERSION.".into(),
                color: "red".into(),
            }],
        )?;
        let newer = w.submit_original(&session.id)?;
        assert_ne!(newer.id, fixed.id);
        let reopened = service(&w, secrets.clone())?;
        assert_eq!(reopened.analysis(&record.id)?.review, record.review);
        let receipt = reopened
            .chat_turn(
                &record.id,
                "解释第一条修改".into(),
                format!("turn-{}", protocol.replace('_', "-")),
                false,
            )
            .await?;
        assert_eq!(
            receipt.context_mode,
            if protocol == "responses" {
                "remote_continue"
            } else {
                "manual_history"
            }
        );
        let updated = reopened.analysis(&record.id)?;
        assert_eq!(updated.review, record.review);
        assert_eq!(updated.sources[0].version_id, fixed.id);
        for format in ["md", "docx"] {
            reopened.export(&record.id, format, &root.join("exports"))?;
        }
        ids.push(record.id);
    }
    println!(
        "{}",
        serde_json::json!({"ok":true,"root":root,"analysisIds":ids,"checks":["two HTTP protocols","evidence validation","overlap and ambiguous duplicates","immutable colored snapshot","restart","chat continuation","md/docx exports"]})
    );
    Ok(())
}
