//! Verify the full installer resources against the actual production data layer.
//! This creates synthetic data only in a new, explicitly supplied directory.
use std::{collections::HashSet, fs, path::PathBuf};
use writing_core::{ImportItem, TextRun, WritingService};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let resources = PathBuf::from(args.next().ok_or("需要完整版题目资源目录")?);
    let root = PathBuf::from(args.next().ok_or("需要不存在的独立测试目录")?);
    if root.exists() {
        return Err("测试目录已存在，未读取或改写其中的数据".into());
    }
    if !resources.is_dir() || args.next().is_some() {
        return Err("用法：full_questions_smoke <题目资源目录> <新测试目录>".into());
    }
    let service = WritingService::new(root.join("fresh"))?;
    let first = service.seed_questions(&resources)?;
    assert_eq!(first.imported.len(), 156);
    assert!(first.skipped.is_empty());
    let questions = service.list_questions()?;
    assert_eq!(
        questions.iter().filter(|q| q.task_type == "task1").count(),
        78
    );
    assert_eq!(
        questions.iter().filter(|q| q.task_type == "task2").count(),
        78
    );
    assert_eq!(
        questions
            .iter()
            .filter(|q| q.title.contains("学术类"))
            .count(),
        128
    );
    assert_eq!(
        questions
            .iter()
            .filter(|q| q.title.contains("培训类"))
            .count(),
        28
    );
    for question in &questions {
        assert!(service
            .question_image(&question.id)?
            .starts_with("data:image/png;base64,"));
    }
    let academic = questions
        .iter()
        .find(|q| q.title.contains("学术类"))
        .unwrap();
    let general = questions
        .iter()
        .find(|q| q.title.contains("培训类"))
        .unwrap();
    let mut essays = Vec::new();
    for question in [academic, general] {
        let essay = service.create_from_question(&question.id)?;
        service.save(
            &essay.id,
            0,
            vec![TextRun {
                text: "A synthetic essay used only to verify the full installer.".into(),
                color: "black".into(),
            }],
        )?;
        service.submit_original(&essay.id)?;
        let dir = root.join("fresh").join(&essay.folder);
        essays.push((
            essay.id.clone(),
            dir.clone(),
            fs::read(dir.join("manifest.json")).map_err(|e| e.to_string())?,
            service.session_question_image(&essay.id)?,
        ));
    }
    let repeat = service.seed_questions(&resources)?;
    assert!(repeat.imported.is_empty());
    assert_eq!(repeat.skipped.len(), 156);
    let custom = service
        .import_images(vec![ImportItem {
            path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets/vision-test.png")
                .display()
                .to_string(),
            task_type: "task2".into(),
            title: Some("自行导入的合成测试题".into()),
        }])?
        .imported
        .into_iter()
        .next()
        .ok_or("自导入测试题失败")?;
    service.delete_question(&academic.id)?;
    let expected: HashSet<_> = service
        .list_questions()?
        .into_iter()
        .map(|q| q.id)
        .collect();
    drop(service);
    let reopened = WritingService::new(root.join("fresh"))?;
    let after_restart = reopened.seed_questions(&resources)?;
    assert!(after_restart.imported.is_empty());
    assert_eq!(after_restart.skipped.len(), 156);
    assert!(after_restart
        .skipped
        .iter()
        .any(|s| s.reason.contains("不自动恢复")));
    let actual: HashSet<_> = reopened
        .list_questions()?
        .into_iter()
        .map(|q| q.id)
        .collect();
    assert_eq!(actual, expected);
    assert!(actual.contains(&custom.id));
    assert_eq!(reopened.list()?.len(), 2);
    for (id, dir, manifest, image) in &essays {
        assert_eq!(
            fs::read(dir.join("manifest.json")).map_err(|e| e.to_string())?,
            *manifest
        );
        assert_eq!(reopened.session_question_image(id)?, *image);
    }

    // Upgrade the published eight-question installation without touching its essay.
    let legacy = WritingService::new(root.join("existing"))?;
    let old_assets =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/questions");
    let old = legacy.seed_questions(&old_assets)?.imported;
    assert_eq!(old.len(), 8);
    let essay = legacy.create_from_question(&old[0].id)?;
    let manifest_path = root
        .join("existing")
        .join(&essay.folder)
        .join("manifest.json");
    let before = fs::read(&manifest_path).map_err(|e| e.to_string())?;
    let snapshot = legacy.session_question_image(&essay.id)?;
    let added = legacy.seed_questions(&resources)?;
    let all = legacy.list_questions()?;
    assert!(old.iter().all(|q| all.iter().any(|v| v.id == q.id)));
    assert_eq!(all.len(), old.len() + added.imported.len());
    assert!(legacy.seed_questions(&resources)?.imported.is_empty());
    assert_eq!(fs::read(manifest_path).map_err(|e| e.to_string())?, before);
    assert_eq!(legacy.session_question_image(&essay.id)?, snapshot);
    println!("PASS: 156 bundled questions; 128 Academic + 28 GT; 78 Task 1 + 78 Task 2");
    println!(
        "PASS: all images readable; sample questions open as essays; repeat launch is idempotent"
    );
    println!(
        "PASS: custom questions, intentional deletions and existing essay snapshots preserved"
    );
    println!("Synthetic data only: {}", root.display());
    Ok(())
}
