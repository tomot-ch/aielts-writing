use crate::{Result, TextRun};
use std::path::{Path, PathBuf};
pub trait WorkspaceStore: Send + Sync {
    fn create_dir(&self, path: &Path) -> Result<()>;
    fn children(&self, path: &Path) -> Result<Vec<PathBuf>>;
    fn read_bytes(&self, path: &Path) -> Result<Vec<u8>>;
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn rename(&self, source: &Path, target: &Path) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
}
pub trait DocumentExporter: Send + Sync {
    fn export(&self, path: &Path, title: &str, runs: &[TextRun]) -> Result<()>;
}
pub trait Clock: Send + Sync {
    fn now(&self) -> String;
    fn local_date(&self) -> String;
}

pub trait SecretStore: Send + Sync {
    fn set(&self, reference: &str, secret: &str) -> Result<()>;
    fn get(&self, reference: &str) -> Result<String>;
}
#[async_trait::async_trait]
pub trait ModelGateway: Send + Sync {
    /// Best-effort cleanup of synthetic capability probes. Not a user-data delete API.
    async fn discard_probe_response(&self, _config: &crate::ConfigRef, _key: &str, _id: &str) {}
    async fn generate(
        &self,
        config: &crate::ConfigRef,
        key: &str,
        request: crate::context::GenerationRequest,
    ) -> std::result::Result<crate::context::GenerationResult, crate::context::GatewayError> {
        use crate::context::*;
        if request.previous_response_id.is_some() {
            return Err(GatewayError::new(GatewayErrorKind::UnsupportedContinuation, "此网关不支持远端接续"));
        }
        self.complete(config, key, request.messages).await.map(GenerationResult::text)
            .map_err(|e| GatewayError::new(GatewayErrorKind::Rejected, e))
    }
    async fn models(&self, _config: &crate::ConfigRef, _key: &str) -> Result<Vec<String>> {
        Err("该服务未提供模型列表，请手动填写模型名称".into())
    }
    async fn complete(
        &self,
        config: &crate::ConfigRef,
        key: &str,
        messages: Vec<serde_json::Value>,
    ) -> Result<String>;
}
