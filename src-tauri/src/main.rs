use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{Emitter, Manager, State};
mod secrets;
mod workspace;
use secrets::PlatformSecrets;
use workspace::{ServiceLease, WorkspaceGate};
use writing_core::ports::WorkspaceStore;
use writing_core::{
    ImportItem, ImportResult, Question, SaveReceipt, SessionSummary, SessionView, Startup, TextRun,
    VersionRef, WritingService,
};

struct AppState {
    config: PathBuf,
    service: Mutex<Option<WritingService>>,
    ai: Mutex<Option<writing_core::AiService>>,
    allow_exit: AtomicBool,
    gate: WorkspaceGate,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootConfig {
    schema_version: u32,
    root: String,
}
fn get_service(state: &State<'_, AppState>) -> Result<ServiceLease<WritingService>, String> {
    let lease = state.gate.operation()?;
    let service = state
        .service
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("请先确认数据目录")?;
    Ok(ServiceLease {
        service,
        _lease: lease,
    })
}
#[tauri::command]
fn startup(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Startup, String> {
    let recommended = app
        .path()
        .document_dir()
        .map_err(|e| e.to_string())?
        .join("雅思写作");
    let s = state.service.lock().map_err(|e| e.to_string())?;
    Ok(Startup {
        configured: s.is_some(),
        root: s.as_ref().map(|s| s.root().display().to_string()),
        recommended_root: recommended.display().to_string(),
    })
}
#[tauri::command]
async fn set_data_root(
    state: State<'_, AppState>,
    path: String,
    migrate: bool,
) -> Result<String, String> {
    let _migration = state.gate.migration()?;
    let target = workspace::canonical_target(Path::new(&path))?;
    let mut guard = state.service.lock().map_err(|e| e.to_string())?;
    let mut ai_guard = state.ai.lock().map_err(|e| e.to_string())?;
    if let Some(ai) = ai_guard.as_ref() {
        if ai.has_active_jobs()? {
            return Err("有 AI 分析任务正在进行，请等待完成或取消任务后再迁移".into());
        }
    }
    let prepare = |root: &Path| -> Result<(WritingService, writing_core::AiService), String> {
        let service = WritingService::new(root.to_path_buf())?;
        let ai = make_ai(service.clone())?;
        writing_core::infrastructure::FileWorkspace.write_atomic(
            &state.config,
            &serde_json::to_vec_pretty(&RootConfig {
                schema_version: 1,
                root: root.display().to_string(),
            })
            .map_err(|e| e.to_string())?,
        )?;
        Ok((service, ai))
    };
    let (service, ai) = if let Some(old) = guard.as_ref() {
        let old_root = fs::canonicalize(old.root()).map_err(|e| e.to_string())?;
        if old_root == target {
            return Ok(target.display().to_string());
        }
        if !migrate {
            return Err("更改已有数据目录需要安全迁移".into());
        }
        workspace::migrate(&old_root, &target, prepare)?
    } else {
        prepare(&target)?
    };
    *guard = Some(service);
    *ai_guard = Some(ai);
    Ok(target.display().to_string())
}
#[tauri::command]
fn seed_questions(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    let path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("resources/questions");
    get_service(&state)?.seed_questions(&path)
}
#[tauri::command]
fn list_questions(state: State<'_, AppState>) -> Result<Vec<Question>, String> {
    get_service(&state)?.list_questions()
}
#[tauri::command]
fn question_image(state: State<'_, AppState>, question_id: String) -> Result<String, String> {
    get_service(&state)?.question_image(&question_id)
}
#[tauri::command]
fn session_question_image(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<String, String> {
    get_service(&state)?.session_question_image(&session_id)
}
#[tauri::command]
fn delete_question(state: State<'_, AppState>, question_id: String) -> Result<(), String> {
    let _change = state.gate.exclusive("删除题目")?;
    let service = state
        .service
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("请先确认数据目录")?;
    service.delete_question(&question_id)
}
#[tauri::command]
fn delete_session(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let _change = state.gate.exclusive("删除作文")?;
    let ai = state
        .ai
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("请先确认数据目录")?;
    ai.delete_session(&session_id)
}
#[tauri::command]
fn import_images(
    state: State<'_, AppState>,
    items: Vec<ImportItem>,
) -> Result<ImportResult, String> {
    get_service(&state)?.import_images(items)
}
#[tauri::command]
fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionSummary>, String> {
    get_service(&state)?.list()
}
#[tauri::command]
fn create_quick(
    state: State<'_, AppState>,
    title: Option<String>,
) -> Result<SessionSummary, String> {
    get_service(&state)?.create_quick(title)
}
#[tauri::command]
fn create_from_question(
    state: State<'_, AppState>,
    question_id: String,
) -> Result<SessionSummary, String> {
    get_service(&state)?.create_from_question(&question_id)
}
#[tauri::command]
fn open_session(state: State<'_, AppState>, session_id: String) -> Result<SessionView, String> {
    get_service(&state)?.open(&session_id)
}
#[tauri::command]
fn save_session(
    state: State<'_, AppState>,
    session_id: String,
    expected_revision: u64,
    runs: Vec<TextRun>,
) -> Result<SaveReceipt, String> {
    get_service(&state)?.save(&session_id, expected_revision, runs)
}
#[tauri::command]
fn submit_original(state: State<'_, AppState>, session_id: String) -> Result<VersionRef, String> {
    get_service(&state)?.submit_original(&session_id)
}
#[tauri::command]
fn begin_original_edit(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    get_service(&state)?.begin_original_edit(&session_id)
}
#[tauri::command]
fn begin_revision(
    state: State<'_, AppState>,
    session_id: String,
    source_version_id: String,
) -> Result<VersionRef, String> {
    get_service(&state)?.begin_revision(&session_id, &source_version_id)
}
#[tauri::command]
fn finish_revision(state: State<'_, AppState>, session_id: String) -> Result<VersionRef, String> {
    get_service(&state)?.finish_revision(&session_id)
}
#[tauri::command]
fn rename_session(
    state: State<'_, AppState>,
    session_id: String,
    title: String,
) -> Result<SessionSummary, String> {
    get_service(&state)?.rename(&session_id, &title)
}
#[tauri::command]
fn set_timer(
    state: State<'_, AppState>,
    session_id: String,
    enabled: bool,
) -> Result<SessionView, String> {
    get_service(&state)?.set_timer(&session_id, enabled)
}
#[tauri::command]
fn export_docx(
    state: State<'_, AppState>,
    session_id: String,
    version_id: String,
    destination: String,
) -> Result<String, String> {
    get_service(&state)?
        .export_docx(&session_id, &version_id, Path::new(&destination))
        .map(|p| p.display().to_string())
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle, state: State<'_, AppState>) {
    state.allow_exit.store(true, Ordering::SeqCst);
    app.exit(0)
}

#[tauri::command]
fn acceptance_mode() -> bool {
    cfg!(feature = "acceptance-fixture")
}
fn make_ai(s: WritingService) -> Result<writing_core::AiService, String> {
    use std::sync::Arc;
    use writing_core::infrastructure::*;
    writing_core::AiService::with_ports(
        s,
        Arc::new(FileWorkspace),
        Arc::new(DocxExporter),
        Arc::new(SystemClock),
        Arc::new(PlatformSecrets),
        Arc::new(writing_core::model_gateway::OpenAiGateway::default()),
    )
}
fn get_ai(s: &State<'_, AppState>) -> Result<ServiceLease<writing_core::AiService>, String> {
    let lease = s.gate.operation()?;
    let service =
        s.ai.lock()
            .map_err(|_| "AI 状态锁失败")?
            .clone()
            .ok_or("请先确认数据目录")?;
    Ok(ServiceLease {
        service,
        _lease: lease,
    })
}
#[tauri::command]
fn ai_configs(state: State<'_, AppState>) -> Result<Vec<writing_core::ConfigRef>, String> {
    get_ai(&state)?.configs()
}
#[tauri::command]
fn ai_missing_credentials(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    get_ai(&state)?.missing_credentials()
}
#[tauri::command]
fn ai_preferences(state: State<'_, AppState>) -> Result<writing_core::AiPreferences, String> {
    get_ai(&state)?.preferences()
}
#[tauri::command]
fn ai_save_preferences(
    state: State<'_, AppState>,
    preferences: writing_core::AiPreferences,
) -> Result<writing_core::AiPreferences, String> {
    get_ai(&state)?.save_preferences(preferences)
}
#[tauri::command]
async fn ai_start_selected(
    state: State<'_, AppState>,
    items: Vec<writing_core::SelectedVersion>,
    many: bool,
) -> Result<writing_core::JobRef, String> {
    get_ai(&state)?.start_selected(items, many)
}
#[tauri::command]
async fn ai_start_conversation(
    state: State<'_, AppState>,
    session_id: String,
    message: String,
) -> Result<writing_core::JobRef, String> {
    get_ai(&state)?.start_conversation(&session_id, message)
}
#[tauri::command]
fn ai_conversation_jobs(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<writing_core::ConversationJob>, String> {
    get_ai(&state)?.conversation_jobs(&session_id)
}
#[tauri::command]
async fn ai_models(
    state: State<'_, AppState>,
    draft: writing_core::ConfigDraft,
) -> Result<Vec<String>, String> {
    get_ai(&state)?.available_models(draft).await
}
#[tauri::command]
async fn ai_test(
    state: State<'_, AppState>,
    draft: writing_core::ConfigDraft,
    vision: bool,
) -> Result<writing_core::CapabilityResult, String> {
    get_ai(&state)?.test(draft, vision).await
}
#[tauri::command]
fn ai_save_config(
    state: State<'_, AppState>,
    draft: writing_core::ConfigDraft,
) -> Result<writing_core::ConfigRef, String> {
    get_ai(&state)?.save_config(draft)
}
#[tauri::command]
fn ai_prompts(state: State<'_, AppState>) -> Result<Vec<writing_core::PromptRef>, String> {
    get_ai(&state)?.prompts()
}
#[tauri::command]
fn ai_save_prompt(
    state: State<'_, AppState>,
    draft: writing_core::PromptDraft,
) -> Result<writing_core::PromptRef, String> {
    get_ai(&state)?.save_prompt(draft)
}
#[tauri::command]
fn ai_delete_prompt(state: State<'_, AppState>, id: String) -> Result<(), String> {
    get_ai(&state)?.delete_prompt(&id)
}
#[tauri::command]
async fn ai_start(
    state: State<'_, AppState>,
    input: writing_core::AnalyzeInput,
) -> Result<writing_core::JobRef, String> {
    get_ai(&state)?.start(input)
}
#[tauri::command]
fn ai_job(state: State<'_, AppState>, id: String) -> Result<writing_core::JobState, String> {
    get_ai(&state)?.job(&id)
}
#[tauri::command]
fn ai_jobs(state: State<'_, AppState>) -> Result<Vec<writing_core::JobState>, String> {
    get_ai(&state)?.jobs()
}
#[tauri::command]
fn ai_many_jobs(state: State<'_, AppState>) -> Result<Vec<writing_core::JobState>, String> {
    get_ai(&state)?.many_jobs()
}
#[tauri::command]
fn ai_cancel(state: State<'_, AppState>, id: String) -> Result<writing_core::JobState, String> {
    get_ai(&state)?.cancel(&id)
}
#[tauri::command]
async fn ai_retry(state: State<'_, AppState>, id: String) -> Result<writing_core::JobRef, String> {
    get_ai(&state)?.retry(&id)
}
#[tauri::command]
fn ai_analyses(state: State<'_, AppState>) -> Result<Vec<writing_core::AnalysisRecord>, String> {
    get_ai(&state)?.analyses()
}
#[tauri::command]
fn ai_analysis(
    state: State<'_, AppState>,
    id: String,
) -> Result<writing_core::AnalysisRecord, String> {
    get_ai(&state)?.analysis(&id)
}
#[tauri::command]
async fn ai_chat(
    state: State<'_, AppState>,
    id: String,
    message: String,
    client_turn_id: String,
) -> Result<writing_core::ChatTurnReceipt, String> {
    get_ai(&state)?.chat_turn(&id, message, client_turn_id, false).await
}
#[tauri::command]
async fn ai_test_context(state: State<'_, AppState>, draft: writing_core::ConfigDraft) -> Result<writing_core::ContextCapability, String> {
    get_ai(&state)?.test_context(draft).await
}
#[tauri::command]
fn ai_pending_turn(state: State<'_, AppState>, id: String) -> Result<Option<writing_core::PendingChatTurn>, String> {
    get_ai(&state)?.pending_chat_turn(&id)
}
#[tauri::command]
async fn ai_retry_turn(state: State<'_, AppState>, id: String, client_turn_id: String) -> Result<writing_core::ChatTurnReceipt, String> {
    let ai = get_ai(&state)?;
    let pending = ai.pending_chat_turn(&id)?.ok_or("没有待处理的聊天请求")?;
    if pending.client_turn_id != client_turn_id { return Err("待处理请求已变化，请重新加载".into()); }
    ai.chat_turn(&id, pending.message, client_turn_id, true).await
}
#[tauri::command]
fn ai_issues(state: State<'_, AppState>) -> Result<Vec<writing_core::IssueRecord>, String> {
    get_ai(&state)?.issues()
}
#[tauri::command]
fn ai_edit_issue(
    state: State<'_, AppState>,
    id: String,
    text: String,
    remove: bool,
) -> Result<writing_core::IssueRecord, String> {
    get_ai(&state)?.edit_issue(&id, text, remove)
}
#[tauri::command]
fn ai_export(
    state: State<'_, AppState>,
    id: String,
    format: String,
    destination: String,
) -> Result<String, String> {
    get_ai(&state)?.export(&id, &format, Path::new(&destination))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config = app
                .path()
                .app_config_dir()
                .map_err(std::io::Error::other)?
                .join("root.json");
            let service = fs::read(&config)
                .ok()
                .and_then(|v| serde_json::from_slice::<RootConfig>(&v).ok())
                .and_then(|c| WritingService::new(PathBuf::from(c.root)).ok());
            let ai = service
                .clone()
                .map(make_ai)
                .transpose()
                .map_err(std::io::Error::other)?;
            app.manage(AppState {
                config,
                service: Mutex::new(service),
                ai: Mutex::new(ai),
                allow_exit: AtomicBool::new(false),
                gate: WorkspaceGate::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            startup,
            set_data_root,
            seed_questions,
            list_sessions,
            list_questions,
            question_image,
            session_question_image,
            delete_question,
            delete_session,
            import_images,
            create_quick,
            create_from_question,
            open_session,
            save_session,
            submit_original,
            begin_original_edit,
            begin_revision,
            finish_revision,
            rename_session,
            set_timer,
            export_docx,
            quit_app,
            ai_configs,
            ai_missing_credentials,
            ai_preferences,
            ai_save_preferences,
            ai_start_selected,
            ai_start_conversation,
            ai_conversation_jobs,
            ai_test,
            ai_models,
            ai_save_config,
            ai_prompts,
            ai_save_prompt,
            ai_delete_prompt,
            ai_start,
            ai_job,
            ai_jobs,
            ai_many_jobs,
            ai_cancel,
            ai_retry,
            ai_analyses,
            ai_analysis,
            ai_chat,
            ai_test_context,
            ai_pending_turn,
            ai_retry_turn,
            ai_issues,
            ai_edit_issue,
            ai_export,
            acceptance_mode
        ])
        .build(tauri::generate_context!())
        .expect("failed to build application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !app.state::<AppState>().allow_exit.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    let _ = app.emit("before-quit", ());
                }
            }
        });
}
