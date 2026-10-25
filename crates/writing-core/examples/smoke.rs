use std::path::PathBuf;
use writing_core::{TextRun, WritingService};
fn main() -> Result<(), String> {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("需要数据目录路径")?;
    let assets =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/questions");
    let service = WritingService::new(root.clone())?;
    let imported = service.seed_questions(&assets)?;
    println!(
        "题库：{} 张，新增：{} 张",
        service.list_questions()?.len(),
        imported.imported.len()
    );
    let question = service
        .list_questions()?
        .into_iter()
        .find(|q| q.task_type == "task1")
        .ok_or("缺少 Task 1")?;
    let session = service.create_from_question(&question.id)?;
    let receipt = service.save(
        &session.id,
        0,
        vec![TextRun {
            text: "The chart shows a steady increase. ".into(),
            color: "black".into(),
        }],
    )?;
    let original = service.submit_original(&session.id)?;
    let _rev = service.begin_revision(&session.id, &original.id)?;
    service.save(
        &session.id,
        receipt.revision + 2,
        vec![
            TextRun {
                text: "The chart shows a steady increase. ".into(),
                color: "black".into(),
            },
            TextRun {
                text: "The change is most visible after 2010.".into(),
                color: "blue".into(),
            },
        ],
    )?;
    let finished = service.finish_revision(&session.id)?;
    let export = service.export_docx(&session.id, &finished.id, &root.join("导出示例"))?;
    println!("作文：{}", root.join(&session.folder).display());
    println!("Word：{}", export.display());
    Ok(())
}
