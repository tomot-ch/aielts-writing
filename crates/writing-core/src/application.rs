use crate::{domain::*, ports::*, Result};
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub struct WritingService {
    root: PathBuf,
    store: std::sync::Arc<dyn WorkspaceStore>,
    exporter: std::sync::Arc<dyn DocumentExporter>,
    clock: std::sync::Arc<dyn Clock>,
    transaction: std::sync::Arc<std::sync::Mutex<()>>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OriginalCommit {
    before_revision: u64,
    document: VersionDoc,
    version: VersionRef,
    manifest: Manifest,
}
impl WritingService {
    pub fn with_ports(
        root: PathBuf,
        store: std::sync::Arc<dyn WorkspaceStore>,
        exporter: std::sync::Arc<dyn DocumentExporter>,
        clock: std::sync::Arc<dyn Clock>,
    ) -> Result<Self> {
        store.create_dir(&root)?;
        for dir in [
            "题库",
            "总问题归纳/单篇问题记录",
            "总问题归纳/总体分析报告",
            "AI相关/prompts",
            "回收站/作文",
            "回收站/题目",
        ] {
            store.create_dir(&root.join(dir))?;
        }
        let service = Self {
            root,
            store,
            exporter,
            clock,
            transaction: std::sync::Arc::new(std::sync::Mutex::new(())),
        };
        service.recover_pending_originals()?;
        service.rebuild_index()?;
        Ok(service)
    }
    fn instant(&self) -> chrono::DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(&self.clock.now())
            .expect("Clock must return ISO 8601")
            .with_timezone(&Utc)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn persist<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        self.store.write_atomic(
            path,
            &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
        )
    }
    fn read<T: for<'a> Deserialize<'a>>(&self, path: &Path) -> Result<T> {
        serde_json::from_slice(&self.store.read_bytes(path)?)
            .map_err(|e| format!("文件损坏 {}: {}", path.display(), e))
    }

    fn find_session_unrecovered(&self, session_id: &str) -> Result<(PathBuf, Manifest)> {
        if !session_id.starts_with("essay-")
            || session_id.contains('/')
            || session_id.contains('\\')
        {
            return Err("无效作文 ID".into());
        }
        for item in self.store.children(&self.root)? {
            let path = item.join("manifest.json");
            if self.store.exists(&path) {
                if let Ok(m) = self.read::<Manifest>(&path) {
                    if m.session_id == session_id {
                        return Ok((path.parent().unwrap().to_path_buf(), m));
                    }
                }
            }
        }
        Err("作文不存在".into())
    }
    pub(crate) fn find_session(&self, session_id: &str) -> Result<(PathBuf, Manifest)> {
        self.find_session_unrecovered(session_id)
    }
    fn find_session_for_write(&self, session_id: &str) -> Result<(PathBuf, Manifest)> {
        let (dir, m) = self.find_session(session_id)?;
        let (m, _) = self.recover_original(&dir, m)?;
        Ok((dir, m))
    }
    fn recover_pending_originals(&self) -> Result<()> {
        for dir in self.store.children(&self.root)? {
            if self.store.exists(&dir.join("pending-original.json")) {
                let m = self.read(&dir.join("manifest.json"))?;
                self.recover_original(&dir, m)?;
            }
        }
        Ok(())
    }
    fn recover_original(
        &self,
        dir: &Path,
        current: Manifest,
    ) -> Result<(Manifest, Option<VersionRef>)> {
        let journal = dir.join("pending-original.json");
        if !self.store.exists(&journal) {
            return Ok((current, None));
        }
        let pending: OriginalCommit = self.read(&journal)?;
        if pending.manifest.session_id != current.session_id
            || pending.version.session_id != current.session_id
            || pending.document.version_id != pending.version.id
            || !pending.version.path.starts_with("versions/original")
            || pending.version.path.contains("..")
            || pending.version.path.contains('\\')
        {
            return Err("初稿提交记录无效，请保留文件并恢复备份".into());
        }
        let already_committed = current.versions.iter().any(|v| v.id == pending.version.id);
        if !already_committed && current.revision != pending.before_revision {
            return Err("初稿提交记录与当前修订冲突，请保留文件并恢复备份".into());
        }
        let path = dir.join(&pending.version.path);
        if self.store.exists(&path) {
            let stored: VersionDoc = self.read(&path)?;
            if stored != pending.document {
                return Err("初稿快照与待提交记录不一致，未覆盖原文件".into());
            }
        } else {
            self.persist(&path, &pending.document)?;
        }
        let manifest = if already_committed {
            current
        } else {
            self.persist(&dir.join("manifest.json"), &pending.manifest)?;
            pending.manifest
        };
        // The manifest is the commit point; a leftover journal can be replayed.
        let _ = self.store.remove_file(&journal);
        Ok((manifest, Some(pending.version)))
    }
    fn summary(&self, dir: &Path, m: &Manifest) -> SessionSummary {
        let active = self.active_path(dir, m);
        let wc = active
            .and_then(|p| self.read::<VersionDoc>(&p).ok())
            .map(|d| word_count(&d.runs))
            .unwrap_or(0);
        SessionSummary {
            id: m.session_id.clone(),
            title: m.title.clone(),
            question_id: m.question_id.clone(),
            task_type: m.task_type.clone(),
            status: m.status.clone(),
            word_count: wc,
            updated_at: m.updated_at.clone(),
            folder: dir.file_name().unwrap().to_string_lossy().to_string(),
        }
    }
    fn active_path(&self, dir: &Path, m: &Manifest) -> Option<PathBuf> {
        if m.status == "draft" || m.status == "editing_original" {
            Some(dir.join("draft.json"))
        } else if let Some(ref vid) = m.working_version_id {
            m.versions
                .iter()
                .find(|v| &v.id == vid)
                .map(|v| dir.join(&v.path))
        } else {
            m.versions.last().map(|v| dir.join(&v.path))
        }
    }
    pub fn rebuild_index(&self) -> Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();
        for item in self.store.children(&self.root)? {
            let dir = item;
            let p = dir.join("manifest.json");
            if self.store.exists(&p) {
                if let Ok(m) = self.read::<Manifest>(&p) {
                    sessions.push(self.summary(&dir, &m));
                }
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        self.persist(
            &self.root.join("index.json"),
            &Index {
                schema_version: 1,
                sessions: sessions.clone(),
            },
        )?;
        Ok(sessions)
    }
    pub fn list(&self) -> Result<Vec<SessionSummary>> {
        self.rebuild_index()
    }
    pub fn list_questions(&self) -> Result<Vec<Question>> {
        let mut result = Vec::new();
        for item in self.store.children(&self.root.join("题库"))? {
            let path = item.join("question.json");
            if self.store.exists(&path) {
                if let Ok(q) = self.read(&path) {
                    result.push(q)
                }
            }
        }
        result.sort_by(|a: &Question, b| a.title.cmp(&b.title));
        Ok(result)
    }
    pub fn question(&self, id: &str) -> Result<Question> {
        if !id.starts_with("q-") || id.contains('/') || id.contains('\\') {
            return Err("无效题目 ID".into());
        }
        self.read(&self.root.join("题库").join(id).join("question.json"))
    }
    pub fn question_image(&self, id: &str) -> Result<String> {
        let q = self.question(id)?;
        let bytes = self
            .store
            .read_bytes(&self.root.join("题库").join(id).join(&q.image_file))?;
        Self::image_data_url(&q, bytes)
    }
    fn image_data_url(q: &Question, bytes: Vec<u8>) -> Result<String> {
        if format!("{:x}", Sha256::digest(&bytes)) != q.sha256 {
            return Err("题目图片校验失败".into());
        }
        let mime = if q.image_file.ends_with(".png") {
            "image/png"
        } else if q.image_file.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };
        Ok(format!(
            "data:{};base64,{}",
            mime,
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }
    pub fn session_question_image(&self, sid: &str) -> Result<String> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, m) = self.find_session(sid)?;
        if m.question_id.is_none() {
            return Err("这篇作文没有题目图片".into());
        }
        let q: Question = self.read(&dir.join("question/question.json"))?;
        let bytes = self
            .store
            .read_bytes(&dir.join("question").join(&q.image_file))?;
        Self::image_data_url(&q, bytes)
    }
    pub fn delete_question(&self, qid: &str) -> Result<()> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        self.question(qid)?;
        self.move_to_trash(&self.root.join("题库").join(qid), "题目", qid)
    }
    pub(crate) fn delete_session(&self, sid: &str) -> Result<()> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, _) = self.find_session(sid)?;
        self.move_to_trash(&dir, "作文", sid)?;
        // The directory move is the commit point; index.json is only a cache.
        let _ = self.rebuild_index();
        Ok(())
    }
    fn move_to_trash(&self, source: &Path, kind: &str, id: &str) -> Result<()> {
        let parent = self.root.join("回收站").join(kind);
        self.store.create_dir(&parent)?;
        let target = parent.join(id);
        if self.store.exists(&target) {
            return Err("回收站已有同名记录，未覆盖任何文件".into());
        }
        self.store.rename(source, &target)
    }
    pub fn import_images(&self, items: Vec<ImportItem>) -> Result<ImportResult> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        self.import_images_inner(items)
    }
    fn import_images_inner(&self, items: Vec<ImportItem>) -> Result<ImportResult> {
        let mut imported = Vec::new();
        let mut skipped = Vec::new();
        for item in items {
            match self.import_one(
                Path::new(&item.path),
                &item.task_type,
                item.title.as_deref(),
            ) {
                Ok(Some(q)) => imported.push(q),
                Ok(None) => skipped.push(Skipped {
                    path: item.path,
                    reason: "图片已导入".into(),
                }),
                Err(e) => skipped.push(Skipped {
                    path: item.path,
                    reason: e,
                }),
            }
        }
        Ok(ImportResult { imported, skipped })
    }
    fn import_one(
        &self,
        path: &Path,
        task_type: &str,
        title: Option<&str>,
    ) -> Result<Option<Question>> {
        if !["task1", "task2"].contains(&task_type) {
            return Err("请选择 Task 1 或 Task 2".into());
        }
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !["png", "jpg", "jpeg", "webp"].contains(&ext.as_str()) {
            return Err("只支持 PNG/JPEG/WebP 图片".into());
        }
        let bytes = self.store.read_bytes(path)?;
        if bytes.len() > 30_000_000 {
            return Err("图片超过 30 MB".into());
        }
        let valid = match ext.as_str() {
            "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            "jpg" | "jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
            _ => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]),
        };
        if !valid {
            return Err("图片格式与文件内容不符".into());
        }
        let hash = format!("{:x}", Sha256::digest(&bytes));
        if self.list_questions()?.iter().any(|q| q.sha256 == hash) {
            return Ok(None);
        }
        let name = title.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("导入题目")
        });
        let qid = id("q");
        let qdir = self.root.join("题库").join(&qid);
        self.store.create_dir(&qdir)?;
        let image_file = format!("images/original.{}", ext);
        self.store.write_atomic(&qdir.join(&image_file), &bytes)?;
        let q = Question {
            schema_version: 1,
            id: qid,
            title: safe_name(&name.replace('_', " · ")),
            task_type: task_type.into(),
            image_count: 1,
            source: "local".into(),
            image_file,
            sha256: hash,
            imported_at: self.clock.now(),
        };
        self.persist(&qdir.join("question.json"), &q)?;
        Ok(Some(q))
    }
    pub fn seed_questions(&self, source: &Path) -> Result<ImportResult> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let mut items = Vec::new();
        let deleted_hashes: std::collections::HashSet<String> = self
            .store
            .children(&self.root.join("回收站/题目"))?
            .iter()
            .filter_map(|p| self.read::<Question>(&p.join("question.json")).ok())
            .map(|q| q.sha256)
            .collect();
        let mut deleted = Vec::new();
        if self.store.exists(source) {
            for p in self.store.children(source)? {
                if p.extension().and_then(|x| x.to_str()) == Some("png") {
                    let hash = format!("{:x}", Sha256::digest(self.store.read_bytes(&p)?));
                    if deleted_hashes.contains(&hash) {
                        deleted.push(Skipped {
                            path: p.display().to_string(),
                            reason: "题目已删除，不自动恢复".into(),
                        });
                        continue;
                    }
                    let name = p
                        .file_stem()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    items.push(ImportItem {
                        path: p.to_string_lossy().to_string(),
                        task_type: if name.contains("task2") {
                            "task2"
                        } else {
                            "task1"
                        }
                        .into(),
                        title: None,
                    })
                }
            }
        }
        items.sort_by(|a, b| a.path.cmp(&b.path));
        let mut result = self.import_images_inner(items)?;
        result.skipped.extend(deleted);
        Ok(result)
    }
    pub fn create_quick(&self, title: Option<String>) -> Result<SessionSummary> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let prefix = format!("{} 快速写作", self.clock.local_date());
        let count = self
            .list()?
            .iter()
            .filter_map(|s| s.title.strip_prefix(&prefix)?.trim().parse::<usize>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        self.create(title.unwrap_or(format!("{} {}", prefix, count)), None, None)
    }
    pub fn create_from_question(&self, qid: &str) -> Result<SessionSummary> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let q = self.question(qid)?;
        self.create(
            format!("{} {}", self.clock.local_date(), q.title),
            Some(q.id),
            Some(q.task_type),
        )
    }
    fn create(
        &self,
        title: String,
        qid: Option<String>,
        task: Option<String>,
    ) -> Result<SessionSummary> {
        let sid = id("essay");
        let title = safe_name(&title);
        let folder = format!("{}_{}_{}", self.clock.local_date(), title, &sid[6..14]);
        let dir = self.root.join(folder);
        self.store.create_dir(&dir)?;
        let mut m = Manifest {
            schema_version: 1,
            session_id: sid.clone(),
            title,
            question_id: qid.clone(),
            task_type: task,
            status: "draft".into(),
            original_version_id: None,
            versions: vec![],
            analysis_ids: vec![],
            created_at: self.clock.now(),
            updated_at: self.clock.now(),
            revision: 0,
            timer_enabled: false,
            timer_started_at: None,
            timer_elapsed_seconds: 0,
            working_version_id: None,
        };
        if let Some(qid) = qid {
            let q = self.question(&qid)?;
            let target = dir.join("question").join(&q.image_file);
            let bytes = self
                .store
                .read_bytes(&self.root.join("题库").join(&qid).join(&q.image_file))?;
            self.store.write_atomic(&target, &bytes)?;
            self.persist(&dir.join("question/question.json"), &q)?;
        }
        self.persist(
            &dir.join("draft.json"),
            &VersionDoc {
                schema_version: 1,
                version_id: id("draft"),
                kind: "draft".into(),
                source_version_id: None,
                runs: vec![],
            },
        )?;
        m.updated_at = self.clock.now();
        self.persist(&dir.join("manifest.json"), &m)?;
        self.rebuild_index()?;
        Ok(self.summary(&dir, &m))
    }
    pub fn open(&self, sid: &str) -> Result<SessionView> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        self.open_inner(sid)
    }
    fn open_inner(&self, sid: &str) -> Result<SessionView> {
        let (dir, m) = self.find_session_for_write(sid)?;
        let active = self.active_path(&dir, &m).ok_or("作文正文不存在")?;
        let doc: VersionDoc = self.read(&active)?;
        let remaining = if m.timer_enabled {
            let limit = if m.task_type.as_deref() == Some("task1") {
                1200
            } else {
                2400
            };
            let elapsed = m
                .timer_started_at
                .as_ref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|t| {
                    (self.instant() - t.with_timezone(&Utc))
                        .num_seconds()
                        .max(0) as u64
                })
                .unwrap_or(0)
                + m.timer_elapsed_seconds;
            Some(limit as i64 - elapsed as i64)
        } else {
            None
        };
        Ok(SessionView {
            session: self.summary(&dir, &m),
            active_runs: doc.runs,
            save_revision: m.revision,
            versions: m.versions,
            timer_enabled: m.timer_enabled,
            remaining_seconds: remaining,
            working_version_id: m.working_version_id,
        })
    }
    pub fn save(&self, sid: &str, expected: u64, runs: Vec<TextRun>) -> Result<SaveReceipt> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        validate_runs(&runs)?;
        let (dir, mut m) = self.find_session_for_write(sid)?;
        if m.revision != expected {
            return Err(format!(
                "版本冲突：当前修订 {}，请求修订 {}",
                m.revision, expected
            ));
        }
        let path = if m.status == "draft" || m.status == "editing_original" {
            dir.join("draft.json")
        } else if let Some(ref vid) = m.working_version_id {
            let v = m
                .versions
                .iter()
                .find(|v| &v.id == vid)
                .ok_or("更改版不存在")?;
            dir.join(&v.path)
        } else {
            return Err("初稿已固定；请先进入更改模式".into());
        };
        let mut doc: VersionDoc = self.read(&path)?;
        doc.runs = runs;
        let wc = word_count(&doc.runs);
        self.persist(&path, &doc)?;
        if let Some(ref vid) = m.working_version_id {
            if let Some(v) = m.versions.iter_mut().find(|v| &v.id == vid) {
                v.word_count = wc
            }
        }
        m.revision += 1;
        m.updated_at = self.clock.now();
        self.persist(&dir.join("manifest.json"), &m)?;
        self.rebuild_index()?;
        Ok(SaveReceipt {
            revision: m.revision,
            saved_at: m.updated_at,
            word_count: wc,
        })
    }
    pub fn submit_original(&self, sid: &str) -> Result<VersionRef> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, current) = self.find_session(sid)?;
        let (mut m, recovered) = self.recover_original(&dir, current)?;
        if let Some(version) = recovered {
            self.rebuild_index()?;
            return Ok(version);
        }
        if m.status != "draft" && m.status != "editing_original" {
            return Err("当前不在初稿编辑状态".into());
        }
        let draft: VersionDoc = self.read(&dir.join("draft.json"))?;
        let previous = m.original_version_id.clone();
        let mut n = m.versions.iter().filter(|v| v.kind == "original").count() + 1;
        let mut doc = VersionDoc {
            schema_version: 1,
            version_id: id("ver-original"),
            kind: "original".into(),
            source_version_id: previous.clone(),
            runs: draft.runs,
        };
        let relative = loop {
            let relative = if n == 1 {
                "versions/original.json".to_string()
            } else {
                format!("versions/original-{n:03}.json")
            };
            let path = dir.join(&relative);
            if !self.store.exists(&path) {
                break relative;
            }
            // Adopt an identical orphan left by V2, preserving its ID and bytes.
            let orphan: VersionDoc = self.read(&path)?;
            if !m.versions.iter().any(|v| v.path == relative)
                && orphan.kind == doc.kind
                && orphan.source_version_id == doc.source_version_id
                && orphan.runs == doc.runs
            {
                doc = orphan;
                break relative;
            }
            n += 1;
        };
        let vid = doc.version_id.clone();
        let v = VersionRef {
            id: vid.clone(),
            session_id: sid.into(),
            kind: "original".into(),
            source_version_id: previous,
            created_at: self.clock.now(),
            word_count: word_count(&doc.runs),
            path: relative,
            fixed: true,
        };
        let before_revision = m.revision;
        m.original_version_id = Some(vid);
        m.versions.push(v.clone());
        m.status = "original_submitted".into();
        m.revision += 1;
        m.updated_at = self.clock.now();
        self.persist(
            &dir.join("pending-original.json"),
            &OriginalCommit {
                before_revision,
                document: doc,
                version: v.clone(),
                manifest: m,
            },
        )?;
        let current = self.read(&dir.join("manifest.json"))?;
        self.recover_original(&dir, current)?;
        self.rebuild_index()?;
        Ok(v)
    }
    pub fn begin_original_edit(&self, sid: &str) -> Result<()> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, mut m) = self.find_session_for_write(sid)?;
        if m.status != "original_submitted" || m.working_version_id.is_some() {
            return Err("请先完成当前编辑".into());
        }
        let previous_id = m.original_version_id.clone().ok_or("尚无固定初稿")?;
        let previous = m
            .versions
            .iter()
            .find(|v| v.id == previous_id && v.fixed)
            .ok_or("初稿版本不存在")?;
        let previous_doc: VersionDoc = self.read(&dir.join(&previous.path))?;
        self.persist(
            &dir.join("draft.json"),
            &VersionDoc {
                schema_version: 1,
                version_id: id("draft"),
                kind: "draft".into(),
                source_version_id: Some(previous_id),
                runs: previous_doc.runs,
            },
        )?;
        m.status = "editing_original".into();
        m.revision += 1;
        m.updated_at = self.clock.now();
        self.persist(&dir.join("manifest.json"), &m)?;
        self.rebuild_index()?;
        Ok(())
    }
    pub fn begin_revision(&self, sid: &str, source_version_id: &str) -> Result<VersionRef> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, mut m) = self.find_session_for_write(sid)?;
        if m.original_version_id.is_none() {
            return Err("请先固定初稿".into());
        }
        if m.status != "original_submitted" {
            return Err("请先完成当前初稿编辑".into());
        }
        if m.working_version_id.is_some() {
            return Err("已有进行中的更改版".into());
        }
        let src = m
            .versions
            .iter()
            .find(|v| v.id == source_version_id && v.fixed)
            .ok_or("来源版本不存在或未固定")?
            .clone();
        let src_doc: VersionDoc = self.read(&dir.join(&src.path))?;
        let n = m.versions.iter().filter(|v| v.kind == "revision").count() + 1;
        let vid = id("ver-rev");
        let path = format!("versions/revision-{:03}.json", n);
        self.persist(
            &dir.join(&path),
            &VersionDoc {
                schema_version: 1,
                version_id: vid.clone(),
                kind: "revision".into(),
                source_version_id: Some(src.id.clone()),
                runs: src_doc.runs,
            },
        )?;
        let v = VersionRef {
            id: vid.clone(),
            session_id: sid.into(),
            kind: "revision".into(),
            source_version_id: Some(src.id),
            created_at: self.clock.now(),
            word_count: src.word_count,
            path,
            fixed: false,
        };
        m.versions.push(v.clone());
        m.working_version_id = Some(vid);
        m.status = "revising".into();
        m.revision += 1;
        m.updated_at = self.clock.now();
        self.persist(&dir.join("manifest.json"), &m)?;
        self.rebuild_index()?;
        Ok(v)
    }
    pub fn finish_revision(&self, sid: &str) -> Result<VersionRef> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, mut m) = self.find_session_for_write(sid)?;
        let vid = m.working_version_id.take().ok_or("没有进行中的更改版")?;
        let v = m
            .versions
            .iter_mut()
            .find(|v| v.id == vid)
            .ok_or("更改版不存在")?;
        v.fixed = true;
        let result = v.clone();
        m.status = "original_submitted".into();
        m.revision += 1;
        m.updated_at = self.clock.now();
        self.persist(&dir.join("manifest.json"), &m)?;
        self.rebuild_index()?;
        Ok(result)
    }
    pub fn rename(&self, sid: &str, title: &str) -> Result<SessionSummary> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, mut m) = self.find_session_for_write(sid)?;
        m.title = safe_name(title);
        m.updated_at = self.clock.now();
        self.persist(&dir.join("manifest.json"), &m)?;
        self.rebuild_index()?;
        Ok(self.summary(&dir, &m))
    }
    pub fn set_timer(&self, sid: &str, enabled: bool) -> Result<SessionView> {
        let _guard = self.transaction.lock().map_err(|e| e.to_string())?;
        let (dir, mut m) = self.find_session_for_write(sid)?;
        if enabled && !m.timer_enabled {
            m.timer_enabled = true;
            m.timer_started_at = Some(self.clock.now())
        } else if !enabled && m.timer_enabled {
            if let Some(start) = m.timer_started_at.take() {
                let elapsed = chrono::DateTime::parse_from_rfc3339(&start)
                    .map(|t| {
                        (self.instant() - t.with_timezone(&Utc))
                            .num_seconds()
                            .max(0) as u64
                    })
                    .unwrap_or(0);
                m.timer_elapsed_seconds += elapsed;
            }
            m.timer_enabled = false
        }
        m.updated_at = self.clock.now();
        self.persist(&dir.join("manifest.json"), &m)?;
        self.open_inner(sid)
    }
    pub fn export_docx(&self, sid: &str, vid: &str, destination: &Path) -> Result<PathBuf> {
        let (dir, m) = self.find_session(sid)?;
        let v = m
            .versions
            .iter()
            .find(|v| v.id == vid && v.fixed)
            .ok_or("请选择已固定的稿件版本")?;
        self.store.create_dir(destination)?;
        let doc: VersionDoc = self.read(&dir.join(&v.path))?;
        let suffix = if v.kind == "original" {
            if v.path == "versions/original.json" {
                "初稿".to_string()
            } else {
                v.path
                    .trim_start_matches("versions/")
                    .trim_end_matches(".json")
                    .to_string()
            }
        } else {
            v.path
                .trim_start_matches("versions/")
                .trim_end_matches(".json")
                .to_string()
        };
        let target = destination.join(format!(
            "{}_{}_{}.docx",
            safe_name(&m.title),
            &sid[6..14],
            suffix
        ));
        let temp = target.with_extension("docx.tmp");
        self.exporter.export(&temp, &m.title, &doc.runs)?;
        self.store.rename(&temp, &target)?;
        Ok(target)
    }
}

impl WritingService {
    pub(crate) fn fixed_runs(&self, item: &SelectedVersion) -> Result<Vec<TextRun>> {
        let (dir, manifest) = self.find_session(&item.session_id)?;
        let version = manifest.versions.iter().find(|v| v.id == item.version_id && v.fixed).ok_or("分析只能选择已固定版本")?;
        let doc: VersionDoc = self.read(&dir.join(&version.path))?;
        Ok(doc.runs)
    }
    pub(crate) fn analysis_source(
        &self,
        item: &SelectedVersion,
    ) -> Result<(PathBuf, String, String, Option<(String, String)>)> {
        let (dir, m) = self.find_session(&item.session_id)?;
        let version = m
            .versions
            .iter()
            .find(|v| v.id == item.version_id && v.fixed)
            .ok_or("分析只能选择已固定版本")?;
        let doc: VersionDoc = self.read(&dir.join(&version.path))?;
        let question = if m.question_id.is_some() {
            let q: Question = self.read(&dir.join("question/question.json"))?;
            let bytes = self
                .store
                .read_bytes(&dir.join("question").join(&q.image_file))?;
            if format!("{:x}", Sha256::digest(&bytes)) != q.sha256 {
                return Err("题目快照校验失败".into());
            }
            let mime = if q.image_file.ends_with(".png") {
                "image/png"
            } else if q.image_file.ends_with(".webp") {
                "image/webp"
            } else {
                "image/jpeg"
            };
            Some((
                q.sha256,
                format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ),
            ))
        } else {
            None
        };
        Ok((
            dir,
            m.title,
            doc.runs.iter().map(|r| r.text.as_str()).collect(),
            question,
        ))
    }
}
