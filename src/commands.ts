import type {
  Startup, SessionSummary, SessionView, TextRun, SaveReceipt, VersionRef,
  Question, ImportItem, ImportResult, ConfigDraft, ConfigRef, CapabilityResult,
  AiPreferences, ConversationJob, SelectedVersion, PromptDraft, PromptRef, AnalyzeInput, JobRef, JobState, AnalysisRecord, ChatTurnReceipt, ContextCapability, PendingChatTurn, IssueRecord,
} from './generated/dto';

/** Typed UI boundary. DTO definitions are generated from contracts/v1.schema.json. */
export type Commands = {
  startup: {args: undefined; result: Startup};
  set_data_root: {args: {path: string; migrate: boolean}; result: string};
  seed_questions: {args: undefined; result: ImportResult};
  list_questions: {args: undefined; result: Question[]};
  question_image: {args: {questionId: string}; result: string};
  session_question_image: {args: {sessionId: string}; result: string};
  delete_question: {args: {questionId: string}; result: null};
  delete_session: {args: {sessionId: string}; result: null};
  import_images: {args: {items: ImportItem[]}; result: ImportResult};
  list_sessions: {args: undefined; result: SessionSummary[]};
  create_quick: {args: {title?: string}; result: SessionSummary};
  create_from_question: {args: {questionId: string}; result: SessionSummary};
  open_session: {args: {sessionId: string}; result: SessionView};
  save_session: {args: {sessionId: string; expectedRevision: number; runs: TextRun[]}; result: SaveReceipt};
  submit_original: {args: {sessionId: string}; result: VersionRef};
  begin_original_edit: {args: {sessionId: string}; result: null};
  begin_revision: {args: {sessionId: string; sourceVersionId: string}; result: VersionRef};
  finish_revision: {args: {sessionId: string}; result: VersionRef};
  rename_session: {args: {sessionId: string; title: string}; result: SessionSummary};
  set_timer: {args: {sessionId: string; enabled: boolean}; result: SessionView};
  export_docx: {args: {sessionId: string; versionId: string; destination: string}; result: string};
  quit_app: {args: undefined; result: null};
  acceptance_mode: {args: undefined; result: boolean};
  ai_configs: {args: undefined; result: ConfigRef[]};
  ai_missing_credentials: {args: undefined; result: string[]};
  ai_preferences: {args: undefined; result: AiPreferences};
  ai_save_preferences: {args: {preferences: AiPreferences}; result: AiPreferences};
  ai_start_selected: {args: {items: SelectedVersion[]; many: boolean}; result: JobRef};
  ai_start_conversation: {args: {sessionId: string; message: string}; result: JobRef};
  ai_conversation_jobs: {args: {sessionId: string}; result: ConversationJob[]};
  ai_test: {args: {draft: ConfigDraft; vision: boolean}; result: CapabilityResult};
  ai_models: {args: {draft: ConfigDraft}; result: string[]};
  ai_save_config: {args: {draft: ConfigDraft}; result: ConfigRef};
  ai_prompts: {args: undefined; result: PromptRef[]};
  ai_save_prompt: {args: {draft: PromptDraft}; result: PromptRef};
  ai_delete_prompt: {args: {id: string}; result: null};
  ai_start: {args: {input: AnalyzeInput}; result: JobRef};
  ai_job: {args: {id: string}; result: JobState};
  ai_jobs: {args: undefined; result: JobState[]};
  ai_many_jobs: {args: undefined; result: JobState[]};
  ai_cancel: {args: {id: string}; result: JobState};
  ai_retry: {args: {id: string}; result: JobRef};
  ai_analyses: {args: undefined; result: AnalysisRecord[]};
  ai_analysis: {args: {id: string}; result: AnalysisRecord};
  ai_chat: {args: {id: string; message: string; clientTurnId: string}; result: ChatTurnReceipt};
  ai_test_context: {args: {draft: ConfigDraft}; result: ContextCapability};
  ai_pending_turn: {args: {id: string}; result: PendingChatTurn | null};
  ai_retry_turn: {args: {id: string; clientTurnId: string}; result: ChatTurnReceipt};
  ai_issues: {args: undefined; result: IssueRecord[]};
  ai_edit_issue: {args: {id: string; text: string; remove: boolean}; result: IssueRecord};
  ai_export: {args: {id: string; format: 'md' | 'docx'; destination: string}; result: string};
};
export type CommandName = keyof Commands;
export type CommandArguments<C extends CommandName> = Commands[C]['args'] extends undefined ? [] : [Commands[C]['args']];
export type CommandResult<C extends CommandName> = Commands[C]['result'];
