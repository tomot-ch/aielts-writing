//! End-to-end acceptance against scripts/mock-openai.py on localhost, using synthetic essays only.
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use writing_core::{
    infrastructure::{DocxExporter, FileWorkspace, SystemClock},
    model_gateway::OpenAiGateway,
    ports::SecretStore,
    AiService, AnalyzeInput, ConfigDraft, Result, SelectedVersion, TextRun, WritingService,
};
struct FixtureSecrets(Mutex<HashMap<String, String>>);
impl SecretStore for FixtureSecrets {
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
            .ok_or("fixture secret missing".into())
    }
}
async fn finish(ai: &AiService, id: &str) -> Result<String> {
    for _ in 0..100 {
        let s = ai.job(id)?;
        if s.status == "completed" {
            return s.result_id.ok_or("missing result".into());
        }
        if s.status == "failed" {
            return Err(s.error.unwrap_or("job failed".into()));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Err("job timeout".into())
}
#[tokio::main]
async fn main() -> Result<()> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("pass output root")?;
    if root.exists() {
        return Err("output root already exists; choose a new directory".into());
    }
    let w = WritingService::new(root.clone())?;
    w.seed_questions(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/questions"),
    )?;
    let q = w
        .list_questions()?
        .into_iter()
        .find(|q| q.task_type == "task1")
        .ok_or("no question")?;
    let first = w.create_from_question(&q.id)?;
    w.save(
        &first.id,
        0,
        vec![TextRun {
            text: "The chart shows a clear increase. Overall, the second category stays stable."
                .into(),
            color: "black".into(),
        }],
    )?;
    let first_v = w.submit_original(&first.id)?;
    let second = w.create_quick(None)?;
    w.save(
        &second.id,
        0,
        vec![TextRun {
            text: "A second synthetic essay compares the two trends and ends with a summary."
                .into(),
            color: "black".into(),
        }],
    )?;
    let second_v = w.submit_original(&second.id)?;
    let ai = AiService::with_ports(
        w.clone(),
        Arc::new(FileWorkspace),
        Arc::new(DocxExporter),
        Arc::new(SystemClock),
        Arc::new(FixtureSecrets(Mutex::new(HashMap::new()))),
        Arc::new(OpenAiGateway::default()),
    )?;
    let draft = ConfigDraft {
        id: None,
        name: "本机模拟模型".into(),
        base_url: "http://127.0.0.1:18765/v1".into(),
        api_key: "test-only-key".into(),
        model_name: "fixture-model".into(),
        context_limit: 32000,
        api_protocol: None,
        context_policy: None,
    };
    assert_eq!(ai.available_models(draft.clone()).await?, vec!["fixture-model"]);
    assert!(ai.test(draft.clone(), false).await?.ok);
    assert!(ai.test(draft.clone(), true).await?.ok);
    let config = ai.save_config(draft)?;
    let prompt = ai.prompts()?.remove(0);
    let a = SelectedVersion {
        session_id: first.id,
        version_id: first_v.id,
    };
    let b = SelectedVersion {
        session_id: second.id,
        version_id: second_v.id,
    };
    let one = ai.start(AnalyzeInput {
        items: vec![a.clone()],
        prompt_id: prompt.id.clone(),
        analysis_config_id: config.id.clone(),
        vision_config_id: None,
        record_issues: true,
        many: false,
    })?;
    let one_id = finish(&ai, &one.id).await?;
    ai.chat(&one_id, "请继续解释主题句。".into()).await?;
    let many = ai.start(AnalyzeInput {
        items: vec![a, b],
        prompt_id: prompt.id,
        analysis_config_id: config.id,
        vision_config_id: None,
        record_issues: false,
        many: true,
    })?;
    let many_id = finish(&ai, &many.id).await?;
    let export = root.join("导出示例");
    ai.export(&one_id, "md", &export)?;
    ai.export(&one_id, "docx", &export)?;
    ai.export(&many_id, "md", &export)?;
    let config_file =
        std::fs::read_to_string(root.join("AI相关/configs.json")).map_err(|e| e.to_string())?;
    assert!(!config_file.contains("test-only-key"));
    println!(
        "single={} many={} issues={} source_count={} root={}",
        one_id,
        many_id,
        ai.issues()?.len(),
        ai.analysis(&many_id)?.sources.len(),
        root.display()
    );
    Ok(())
}
