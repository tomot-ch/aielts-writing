pub type Result<T> = std::result::Result<T, String>;
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod models;
pub mod context;
pub mod review;
pub mod ports;
pub use application::WritingService;
pub use domain::*;
#[cfg(test)]
mod test_support;
impl WritingService {
    pub fn new(root: std::path::PathBuf) -> Result<Self> {
        Self::with_ports(
            root,
            std::sync::Arc::new(infrastructure::FileWorkspace),
            std::sync::Arc::new(infrastructure::DocxExporter),
            std::sync::Arc::new(infrastructure::SystemClock),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use domain::id;
    use sha2::{Digest, Sha256};
    use std::{fs, fs::File, path::PathBuf};
    fn read_json<T: for<'a> serde::Deserialize<'a>>(path: &std::path::Path) -> Result<T> {
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(id("writing-test"));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn original_is_immutable_and_revision_survives_reopen() {
        let p = temp();
        let s = WritingService::new(p.clone()).unwrap();
        let a = s.create_quick(None).unwrap();
        let v = s.open(&a.id).unwrap();
        s.save(
            &a.id,
            v.save_revision,
            vec![TextRun {
                text: "First text".into(),
                color: "black".into(),
            }],
        )
        .unwrap();
        let original = s.submit_original(&a.id).unwrap();
        assert!(s.submit_original(&a.id).is_err());
        assert!(s
            .save(&a.id, s.open(&a.id).unwrap().save_revision, vec![])
            .is_err());
        s.begin_revision(&a.id, &original.id).unwrap();
        let x = s.open(&a.id).unwrap();
        s.save(
            &a.id,
            x.save_revision,
            vec![
                TextRun {
                    text: "First text".into(),
                    color: "black".into(),
                },
                TextRun {
                    text: " new".into(),
                    color: "blue".into(),
                },
            ],
        )
        .unwrap();
        let reopened = WritingService::new(p.clone()).unwrap().open(&a.id).unwrap();
        assert_eq!(reopened.active_runs[1].color, "blue");
        s.finish_revision(&a.id).unwrap();
        let (dir, _) = s.find_session(&a.id).unwrap();
        let orig: VersionDoc = read_json(&dir.join("versions/original.json")).unwrap();
        assert_eq!(orig.runs[0].text, "First text");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn confirmed_original_edit_creates_new_fixed_version() {
        let p = temp();
        let s = WritingService::new(p.clone()).unwrap();
        let a = s.create_quick(None).unwrap();
        s.save(
            &a.id,
            0,
            vec![TextRun {
                text: "Before".into(),
                color: "black".into(),
            }],
        )
        .unwrap();
        let first = s.submit_original(&a.id).unwrap();
        assert!(s
            .save(&a.id, s.open(&a.id).unwrap().save_revision, vec![])
            .is_err());
        s.begin_original_edit(&a.id).unwrap();
        let editing = s.open(&a.id).unwrap();
        assert_eq!(editing.session.status, "editing_original");
        assert_eq!(editing.active_runs[0].text, "Before");
        s.save(
            &a.id,
            editing.save_revision,
            vec![TextRun {
                text: "After".into(),
                color: "black".into(),
            }],
        )
        .unwrap();
        let second = s.submit_original(&a.id).unwrap();
        assert_eq!(second.source_version_id.as_deref(), Some(first.id.as_str()));
        assert_ne!(second.path, first.path);
        let (dir, _) = s.find_session(&a.id).unwrap();
        let old: VersionDoc = read_json(&dir.join(&first.path)).unwrap();
        let new: VersionDoc = read_json(&dir.join(&second.path)).unwrap();
        assert_eq!(old.runs[0].text, "Before");
        assert_eq!(new.runs[0].text, "After");
        let reopened = WritingService::new(p.clone()).unwrap().open(&a.id).unwrap();
        assert_eq!(reopened.active_runs[0].text, "After");
        assert_eq!(reopened.versions.len(), 2);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn conflict_and_docx_color() {
        let p = temp();
        let s = WritingService::new(p.clone()).unwrap();
        let a = s.create_quick(None).unwrap();
        s.save(
            &a.id,
            0,
            vec![TextRun {
                text: "Hello".into(),
                color: "red".into(),
            }],
        )
        .unwrap();
        assert!(s.save(&a.id, 0, vec![]).is_err());
        let v = s.submit_original(&a.id).unwrap();
        let out = s.export_docx(&a.id, &v.id, &p.join("exports")).unwrap();
        let mut z = zip::ZipArchive::new(File::open(out).unwrap()).unwrap();
        use std::io::Read;
        let mut content = String::new();
        z.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("DC2626"));
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn seed_images_snapshot_and_index_rebuild() {
        let p = temp();
        let s = WritingService::new(p.clone()).unwrap();
        let assets =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/questions");
        let result = s.seed_questions(&assets).unwrap();
        assert_eq!(result.imported.len(), 8);
        assert_eq!(s.list_questions().unwrap().len(), 8);
        let q = s
            .list_questions()
            .unwrap()
            .into_iter()
            .find(|q| q.task_type == "task1")
            .unwrap();
        let session = s.create_from_question(&q.id).unwrap();
        let (dir, _) = s.find_session(&session.id).unwrap();
        let snapshot = fs::read(dir.join("question").join(&q.image_file)).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(snapshot)), q.sha256);
        fs::write(p.join("index.json"), b"broken").unwrap();
        let reopened = WritingService::new(p.clone()).unwrap();
        assert_eq!(reopened.list().unwrap().len(), 1);
        assert!(!reopened.open(&session.id).unwrap().timer_enabled);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn reseeding_recovers_missing_question_files_without_changing_essays() {
        for missing_count in [1, 8] {
            let root = temp();
            let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../src-tauri/resources/questions");
            let writing = WritingService::new(root.clone()).unwrap();
            writing.seed_questions(&assets).unwrap();
            let questions = writing.list_questions().unwrap();
            let essay = writing.create_from_question(&questions[0].id).unwrap();
            let (essay_dir, _) = writing.find_session(&essay.id).unwrap();
            let manifest = fs::read(essay_dir.join("manifest.json")).unwrap();
            let snapshot = writing.session_question_image(&essay.id).unwrap();

            // Match a cleaned temporary directory: folders remain, files disappear.
            for question in questions.iter().take(missing_count) {
                let dir = root.join("题库").join(&question.id);
                fs::remove_file(dir.join("question.json")).unwrap();
                fs::remove_file(dir.join(&question.image_file)).unwrap();
            }

            let reopened = WritingService::new(root.clone()).unwrap();
            assert_eq!(
                reopened.seed_questions(&assets).unwrap().imported.len(),
                missing_count
            );
            let restored = reopened.list_questions().unwrap();
            assert_eq!(restored.len(), 8);
            for question in questions.iter().skip(missing_count) {
                assert!(restored.iter().any(|q| q.id == question.id));
            }
            assert!(reopened
                .seed_questions(&assets)
                .unwrap()
                .imported
                .is_empty());
            for question in &restored {
                assert!(reopened.question_image(&question.id).is_ok());
            }
            assert_eq!(reopened.list().unwrap().len(), 1);
            assert_eq!(fs::read(essay_dir.join("manifest.json")).unwrap(), manifest);
            assert_eq!(
                reopened.session_question_image(&essay.id).unwrap(),
                snapshot
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn original_commit_recovers_after_manifest_failure_on_retry_and_reopen() {
        for reopen in [false, true] {
            let root = temp();
            let store = std::sync::Arc::new(test_support::FailOnceStore::default());
            let writing = WritingService::with_ports(
                root.clone(),
                store.clone(),
                std::sync::Arc::new(infrastructure::DocxExporter),
                std::sync::Arc::new(infrastructure::SystemClock),
            )
            .unwrap();
            let session = writing.create_quick(None).unwrap();
            writing
                .save(
                    &session.id,
                    0,
                    vec![TextRun {
                        text: "Don't lose this original.".into(),
                        color: "black".into(),
                    }],
                )
                .unwrap();
            let (dir, _) = writing.find_session(&session.id).unwrap();
            store.fail_next("manifest.json");
            assert!(writing.submit_original(&session.id).is_err());
            let snapshot = fs::read(dir.join("versions/original.json")).unwrap();
            assert!(dir.join("pending-original.json").exists());
            let version = if reopen {
                WritingService::new(root.clone())
                    .unwrap()
                    .open(&session.id)
                    .unwrap()
                    .versions
                    .remove(0)
            } else {
                writing.submit_original(&session.id).unwrap()
            };
            let view = writing.open(&session.id).unwrap();
            assert_eq!(view.session.status, "original_submitted");
            assert_eq!(view.versions.len(), 1);
            assert_eq!(view.versions[0].id, version.id);
            assert_eq!(snapshot, fs::read(dir.join(&version.path)).unwrap());
            assert!(!dir.join("pending-original.json").exists());
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn original_commit_adopts_v2_orphan_without_overwriting_it() {
        let root = temp();
        let writing = WritingService::new(root.clone()).unwrap();
        let session = writing.create_quick(None).unwrap();
        let runs = vec![TextRun {
            text: "An orphan original.".into(),
            color: "black".into(),
        }];
        writing.save(&session.id, 0, runs.clone()).unwrap();
        let (dir, _) = writing.find_session(&session.id).unwrap();
        let doc = VersionDoc {
            schema_version: 1,
            version_id: id("ver-original"),
            kind: "original".into(),
            source_version_id: None,
            runs,
        };
        fs::create_dir_all(dir.join("versions")).unwrap();
        let bytes = serde_json::to_vec(&doc).unwrap();
        fs::write(dir.join("versions/original.json"), &bytes).unwrap();
        let version = writing.submit_original(&session.id).unwrap();
        assert_eq!(version.id, doc.version_id);
        assert_eq!(bytes, fs::read(dir.join(&version.path)).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn word_count_matches_shared_examples_across_colored_runs() {
        let examples: serde_json::Value =
            serde_json::from_str(include_str!("../../../contracts/word-count.examples.json"))
                .unwrap();
        for example in examples.as_array().unwrap() {
            let runs: Vec<TextRun> = example["text"]
                .as_str()
                .unwrap()
                .chars()
                .enumerate()
                .map(|(n, c)| TextRun {
                    text: c.to_string(),
                    color: if n % 2 == 0 { "black" } else { "blue" }.into(),
                })
                .collect();
            assert_eq!(
                domain::word_count(&runs),
                example["count"].as_u64().unwrap() as usize,
                "{example}"
            );
        }
    }
    #[test]
    fn deleted_question_keeps_essay_snapshot_and_is_not_reseeded() {
        let root = temp();
        let writing = WritingService::new(root.clone()).unwrap();
        let assets =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/questions");
        writing.seed_questions(&assets).unwrap();
        let q = writing.list_questions().unwrap().remove(0);
        let essay = writing.create_from_question(&q.id).unwrap();
        let image = writing.session_question_image(&essay.id).unwrap();
        let source = fs::read_dir(&assets)
            .unwrap()
            .map(|p| p.unwrap().path())
            .find(|p| format!("{:x}", Sha256::digest(fs::read(p).unwrap())) == q.sha256)
            .unwrap();
        writing.delete_question(&q.id).unwrap();
        assert!(writing.question(&q.id).is_err());
        assert_eq!(writing.list_questions().unwrap().len(), 7);
        assert_eq!(writing.session_question_image(&essay.id).unwrap(), image);
        writing
            .save(
                &essay.id,
                0,
                vec![TextRun {
                    text: "Snapshot essay.".into(),
                    color: "black".into(),
                }],
            )
            .unwrap();
        let version = writing.submit_original(&essay.id).unwrap();
        assert!(writing
            .analysis_source(&SelectedVersion {
                session_id: essay.id.clone(),
                version_id: version.id
            })
            .unwrap()
            .3
            .is_some());
        assert!(root
            .join(format!("回收站/题目/{}/question.json", q.id))
            .exists());
        assert!(source.exists());
        let reopened = WritingService::new(root.clone()).unwrap();
        assert!(reopened
            .seed_questions(&assets)
            .unwrap()
            .imported
            .is_empty());
        assert_eq!(reopened.list_questions().unwrap().len(), 7);
        assert_eq!(reopened.session_question_image(&essay.id).unwrap(), image);
        assert_eq!(
            reopened
                .import_images(vec![ImportItem {
                    path: source.display().to_string(),
                    task_type: q.task_type,
                    title: None
                }])
                .unwrap()
                .imported
                .len(),
            1
        );
        assert_eq!(reopened.list_questions().unwrap().len(), 8);
        assert!(reopened.delete_question("q-../../outside").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn deleted_essay_moves_all_versions_and_updates_index_without_reusing_live_titles() {
        let root = temp();
        let writing = WritingService::new(root.clone()).unwrap();
        let first = writing.create_quick(None).unwrap();
        writing
            .save(
                &first.id,
                0,
                vec![TextRun {
                    text: "Original.".into(),
                    color: "black".into(),
                }],
            )
            .unwrap();
        let original = writing.submit_original(&first.id).unwrap();
        let revision = writing.begin_revision(&first.id, &original.id).unwrap();
        let second = writing.create_quick(None).unwrap();
        writing.delete_session(&first.id).unwrap();
        assert!(writing.open(&first.id).is_err());
        assert_eq!(writing.list().unwrap().len(), 1);
        assert!(writing.open(&second.id).is_ok());
        let trash = root.join("回收站/作文").join(&first.id);
        assert!(trash.join("manifest.json").exists());
        assert!(trash.join("draft.json").exists());
        assert!(trash.join(&original.path).exists());
        assert!(trash.join(&revision.path).exists());
        let index: Index = read_json(&root.join("index.json")).unwrap();
        assert!(index.sessions.iter().all(|s| s.id != first.id));
        let reopened = WritingService::new(root.clone()).unwrap();
        let third = reopened.create_quick(None).unwrap();
        assert!(third.title.ends_with("快速写作 3"));
        assert!(reopened.delete_session(&first.id).is_err());
        assert!(trash.join(&original.path).exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_trash_move_keeps_original_data_and_lists() {
        let root = temp();
        let store = std::sync::Arc::new(test_support::FailOnceStore::default());
        let writing = WritingService::with_ports(
            root.clone(),
            store.clone(),
            std::sync::Arc::new(infrastructure::DocxExporter),
            std::sync::Arc::new(infrastructure::SystemClock),
        )
        .unwrap();
        let assets =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/questions");
        writing.seed_questions(&assets).unwrap();
        let question = writing.list_questions().unwrap().remove(0);
        let essay = writing.create_from_question(&question.id).unwrap();
        store.fail_next_rename();
        assert!(writing.delete_question(&question.id).is_err());
        assert_eq!(writing.list_questions().unwrap().len(), 8);
        assert!(writing.question_image(&question.id).is_ok());
        store.fail_next_rename();
        assert!(writing.delete_session(&essay.id).is_err());
        assert_eq!(writing.list().unwrap().len(), 1);
        assert!(writing.open(&essay.id).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
}
pub mod ai;
pub mod model_gateway;
pub use ai::AiService;
