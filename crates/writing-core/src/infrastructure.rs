use crate::{
    ports::{Clock, DocumentExporter, WorkspaceStore},
    Result, TextRun,
};
use chrono::{Local, Utc};
use std::{
    fs,
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub struct FileWorkspace;
impl WorkspaceStore for FileWorkspace {
    fn create_dir(&self, path: &Path) -> Result<()> {
        fs::create_dir_all(path).map_err(|e| e.to_string())
    }
    fn children(&self, path: &Path) -> Result<Vec<PathBuf>> {
        fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect()
    }
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        fs::read(path).map_err(|e| e.to_string())
    }
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        atomic_write(path, bytes)
    }
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn rename(&self, source: &Path, target: &Path) -> Result<()> {
        fs::rename(source, target).map_err(|e| e.to_string())
    }
    fn remove_file(&self, path: &Path) -> Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> String {
        Utc::now().to_rfc3339()
    }
    fn local_date(&self) -> String {
        Local::now().format("%Y-%m-%d").to_string()
    }
}
pub struct DocxExporter;
impl DocumentExporter for DocxExporter {
    fn export(&self, path: &Path, title: &str, runs: &[TextRun]) -> Result<()> {
        make_docx(path, title, runs)
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4().simple()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| e.to_string())?;
    Ok(())
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn make_docx(path: &Path, title: &str, runs: &[TextRun]) -> Result<()> {
    use zip::write::SimpleFileOptions;
    let f = File::create(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipWriter::new(f);
    let opt = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut add = |name: &str, body: &str| -> Result<()> {
        z.start_file(name, opt).map_err(|e| e.to_string())?;
        z.write_all(body.as_bytes()).map_err(|e| e.to_string())
    };
    add(
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
    )?;
    add(
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
    )?;
    let mut body = format!(
        "<w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr><w:r><w:t>{}</w:t></w:r></w:p>",
        xml(title)
    );
    let colors = |c: &str| match c {
        "blue" => "2563EB",
        "green" => "15803D",
        "red" => "DC2626",
        _ => "202124",
    };
    body.push_str("<w:p>");
    for run in runs {
        for (i, line) in run.text.split('\n').enumerate() {
            if i > 0 {
                body.push_str("</w:p><w:p>")
            };
            if !line.is_empty() {
                body.push_str(&format!("<w:r><w:rPr><w:color w:val=\"{}\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r>",colors(&run.color),xml(line)))
            }
        }
    }
    body.push_str("</w:p>");
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{}<w:sectPr/></w:body></w:document>"#,
        body
    );
    add("word/document.xml", &content)?;
    z.finish().map_err(|e| e.to_string())?;
    Ok(())
}
