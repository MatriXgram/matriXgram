use matrix_sdk::authentication::matrix::MatrixSession as SdkMatrixSession;
use matrix_sdk::Client;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use storage::AppDb;
use types::{storage_keys, Paths};

pub struct PersistedMatrixSession {
    pub homeserver_url: String,
    pub matrix_session: SdkMatrixSession,
}

impl PersistedMatrixSession {
    pub fn new(homeserver_url: String, matrix_session: SdkMatrixSession) -> Self {
        Self {
            homeserver_url,
            matrix_session,
        }
    }
}

pub fn matrix_sdk_store_path(paths: &Paths) -> PathBuf {
    paths.data_file(storage_keys::MATRIX_SDK_STORE_DIR)
}

pub fn prepare_matrix_sdk_store(paths: &Paths) -> Result<PathBuf, String> {
    let store_path = matrix_sdk_store_path(paths);
    fs::create_dir_all(&store_path)
        .map_err(|error| format!("Failed to create Matrix SDK store directory: {error}"))?;
    Ok(store_path)
}

pub async fn load_persisted_session(
    app_db: &Arc<AppDb>,
) -> Result<Option<PersistedMatrixSession>, String> {
    let loaded = app_db.load_persisted_session().await?;
    Ok(
        loaded.map(|(homeserver_url, matrix_session)| PersistedMatrixSession {
            homeserver_url,
            matrix_session,
        }),
    )
}

pub async fn persist_session(
    app_db: &Arc<AppDb>,
    session: &PersistedMatrixSession,
) -> Result<(), String> {
    app_db
        .persist_session(&session.homeserver_url, &session.matrix_session)
        .await
}

pub async fn persist_session_from_client(
    app_db: &Arc<AppDb>,
    client: &Client,
) -> Result<(), String> {
    let session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| String::from("Missing Matrix session while persisting token refresh"))?;

    persist_session(
        app_db,
        &PersistedMatrixSession::new(client.homeserver().to_string(), session),
    )
    .await
}

pub async fn clear_persisted_session(app_db: &Arc<AppDb>) -> Result<(), String> {
    app_db.clear_session().await
}

pub async fn clear_app_cache(app_db: &Arc<AppDb>) -> Result<(), String> {
    app_db.clear_app_cache().await
}

pub async fn clear_app_cache_except_auth(app_db: &Arc<AppDb>) -> Result<(), String> {
    app_db.clear_non_auth_cache().await
}

pub fn clear_matrix_sdk_store(paths: &Paths) -> Result<(), String> {
    let path = matrix_sdk_store_path(paths);
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("Failed to clear Matrix SDK store: {error}"))?;
    }

    Ok(())
}

/// Windows may retain SQLite handles briefly after cancelled tasks drop clients.
pub async fn clear_matrix_sdk_store_with_retry(paths: &Paths) -> Result<(), String> {
    for attempt in 0..20 {
        match clear_matrix_sdk_store(paths) {
            Ok(()) => return Ok(()),
            Err(error) if attempt == 19 => return Err(error),
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    }
    unreachable!()
}

pub async fn prepare_login_store(paths: &Paths, app_db: &Arc<AppDb>) -> Result<PathBuf, String> {
    if load_persisted_session(app_db).await?.is_none() {
        clear_matrix_sdk_store_with_retry(paths).await?;
    }
    prepare_matrix_sdk_store(paths)
}

#[cfg(all(test, windows))]
mod cleanup_tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;
    #[tokio::test]
    async fn cleanup_waits_for_windows_file_handles_to_close() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "matrix-logout-cleanup-{}-{unique}",
            std::process::id()
        ));
        let paths = Paths::new(root.clone(), root.join("cache"));
        let store = prepare_matrix_sdk_store(&paths).unwrap();
        let file = store.join("database.sqlite");
        fs::write(&file, b"test").unwrap();
        let handle = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .unwrap();
        let release = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            drop(handle);
        });
        clear_matrix_sdk_store_with_retry(&paths).await.unwrap();
        release.await.unwrap();
        assert!(!store.exists());
        fs::remove_dir(root).unwrap();
    }
}
