//! Keep API keys in Keychain without opening system password dialogs.
use writing_core::ports::SecretStore;

pub struct PlatformSecrets;

#[cfg(feature = "acceptance-fixture")]
static TEST_SECRETS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, String>>,
> = std::sync::OnceLock::new();

#[cfg(all(target_os = "macos", not(feature = "acceptance-fixture")))]
const SERVICE: &str = "com.local.ieltswriting";
#[cfg(all(target_os = "macos", not(feature = "acceptance-fixture")))]
const REENTER_KEY: &str = "已保存的 API Key 暂时无法读取。请到“语言大模型配置 → 添加模型”重新填写 API Key，测试并保存；无需输入 Mac 登录密码。";

// The legacy login Keychain used by existing V2 installs needs this macOS API
// to suppress its UI. Scope the setting to each operation and serialize access
// because the interaction flag is shared by this process, not the whole system.
#[cfg(all(target_os = "macos", not(feature = "acceptance-fixture")))]
fn without_password_dialog<T>(
    operation: impl FnOnce() -> security_framework::base::Result<T>,
) -> Result<T, String> {
    use security_framework::os::macos::keychain::SecKeychain;
    static OPERATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _operation = OPERATIONS.lock().map_err(|_| "凭据状态锁失败")?;
    let was_allowed =
        SecKeychain::user_interaction_allowed().map_err(|_| "无法确认钥匙串交互状态")?;
    let guard = SecKeychain::disable_user_interaction().map_err(|_| "无法启用静默钥匙串访问")?;
    let result = operation().map_err(|_| "无法静默访问钥匙串".to_owned());
    if !was_allowed {
        // This crate's guard always enables UI on drop; preserve an already-off
        // setting instead. The guard is a zero-sized token with no resources.
        std::mem::forget(guard);
    }
    result
}

impl SecretStore for PlatformSecrets {
    fn set(&self, reference: &str, secret: &str) -> Result<(), String> {
        #[cfg(feature = "acceptance-fixture")]
        {
            TEST_SECRETS
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| "测试凭据锁失败")?
                .insert(reference.into(), secret.into());
            return Ok(());
        }
        #[cfg(all(target_os = "macos", not(feature = "acceptance-fixture")))]
        {
            without_password_dialog(|| {
                security_framework::passwords::set_generic_password(
                    SERVICE,
                    reference,
                    secret.as_bytes(),
                )
            })
            .map_err(|_| {
                "无法静默保存 API Key。请确认登录钥匙串已解锁后重试；已保存的模型配置不会被替换。"
                    .into()
            })
        }
        #[cfg(all(not(target_os = "macos"), not(feature = "acceptance-fixture")))]
        {
            Err("当前平台的安全凭据适配器未实现".into())
        }
    }

    fn get(&self, reference: &str) -> Result<String, String> {
        #[cfg(feature = "acceptance-fixture")]
        {
            return TEST_SECRETS
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| "测试凭据锁失败")?
                .get(reference)
                .cloned()
                .ok_or("验收版重启后需重新填写测试密钥".into());
        }
        #[cfg(all(target_os = "macos", not(feature = "acceptance-fixture")))]
        {
            let bytes = without_password_dialog(|| {
                security_framework::passwords::get_generic_password(SERVICE, reference)
            })
            .map_err(|_| REENTER_KEY.to_owned())?;
            String::from_utf8(bytes).map_err(|_| REENTER_KEY.to_owned())
        }
        #[cfg(all(not(target_os = "macos"), not(feature = "acceptance-fixture")))]
        {
            Err("当前平台的安全凭据适配器未实现".into())
        }
    }
}

#[cfg(all(test, target_os = "macos", not(feature = "acceptance-fixture")))]
mod tests {
    use super::*;
    use security_framework::{base::Error, os::macos::keychain::SecKeychain};

    #[test]
    fn system_dialogs_are_disabled_during_operations_and_state_is_restored_on_error() {
        let before = SecKeychain::user_interaction_allowed().unwrap();
        let result = without_password_dialog(|| {
            assert!(!SecKeychain::user_interaction_allowed().unwrap());
            Err::<(), _>(Error::from_code(-25308))
        });
        assert!(result.is_err());
        assert_eq!(SecKeychain::user_interaction_allowed().unwrap(), before);
        let result = without_password_dialog(|| {
            assert!(!SecKeychain::user_interaction_allowed().unwrap());
            Ok(())
        });
        assert!(result.is_ok());
        assert_eq!(SecKeychain::user_interaction_allowed().unwrap(), before);
    }

    #[test]
    #[ignore = "uses an isolated CLI-owned synthetic item to verify denied access stays silent"]
    fn protected_item_returns_error_instead_of_asking_for_a_system_password() {
        use std::process::Command;
        let account = format!(
            "quiet-protected-smoke-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let added = Command::new("/usr/bin/security")
            .args([
                "add-generic-password",
                "-s",
                SERVICE,
                "-a",
                &account,
                "-w",
                "synthetic-only-not-a-real-key",
                "-T",
                "/usr/bin/security",
            ])
            .output()
            .unwrap();
        assert!(
            added.status.success(),
            "could not create isolated test credential"
        );
        let result = PlatformSecrets.get(&account);
        let removed = Command::new("/usr/bin/security")
            .args(["delete-generic-password", "-s", SERVICE, "-a", &account])
            .output()
            .unwrap();
        assert!(
            removed.status.success(),
            "could not clean up isolated test credential"
        );
        assert!(
            result.is_err(),
            "a credential restricted to the security tool was unexpectedly readable"
        );
        assert!(result.err().unwrap().contains("无需输入 Mac 登录密码"));
    }

    // Explicitly opt in: touches only a uniquely named synthetic credential.
    #[test]
    #[ignore = "uses the real login Keychain with a disposable synthetic key"]
    fn real_keychain_roundtrip_without_password_dialogs() {
        let account = format!(
            "quiet-keychain-smoke-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
        );
        let store = PlatformSecrets;
        let result = (|| {
            store.set(&account, "synthetic-only-not-a-real-key")?;
            let value = store.get(&account)?;
            assert_eq!(value, "synthetic-only-not-a-real-key");
            Ok::<(), String>(())
        })();
        let cleanup = without_password_dialog(|| {
            security_framework::passwords::delete_generic_password(SERVICE, &account)
        });
        result.unwrap();
        cleanup.unwrap();
    }
}
