//! Local STT model download + integrity verification (phase 2.6b —
//! docs/local-stt-plan.md §4). The model weights (~1.5 GB) aren't bundled
//! with the installer; they're pulled from CDN on first "Cihazında"
//! activation and checked against the SHA-256 the server manifest gives —
//! a truncated or corrupted download must never look like a usable model.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum ModelFetchError {
    #[error("download request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server returned {0}")]
    Status(reqwest::StatusCode),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
}

/// Ensures `dest` holds the model at `url` matching `expected_sha256_hex`.
/// Skips the download if `dest` already exists and hashes correctly (the
/// dashboard's hardware-test button gets pressed more than once; the
/// ~1.5 GB model shouldn't be refetched every time). Streams into a
/// `.part` sibling and hashes incrementally so an interrupted download is
/// never mistaken for a valid model — it's only renamed into place after
/// the digest matches.
pub async fn ensure_model(
    http: &reqwest::Client,
    url: &str,
    expected_sha256_hex: &str,
    dest: &Path,
) -> Result<PathBuf, ModelFetchError> {
    if file_matches(dest, expected_sha256_hex).await? {
        return Ok(dest.to_path_buf());
    }

    let response = http.get(url).send().await?;
    if !response.status().is_success() {
        return Err(ModelFetchError::Status(response.status()));
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let part_path = part_path_for(dest);
    let actual_hex = stream_to_file(response, &part_path).await?;

    if !actual_hex.eq_ignore_ascii_case(expected_sha256_hex) {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(ModelFetchError::ChecksumMismatch {
            expected: expected_sha256_hex.to_string(),
            actual: actual_hex,
        });
    }

    tokio::fs::rename(&part_path, dest).await?;
    Ok(dest.to_path_buf())
}

fn part_path_for(dest: &Path) -> PathBuf {
    let mut name = dest
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".part");
    dest.with_file_name(name)
}

async fn file_matches(path: &Path, expected_sha256_hex: &str) -> Result<bool, ModelFetchError> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(false);
    }
    let owned_path = path.to_path_buf();
    let actual = tokio::task::spawn_blocking(move || sha256_file(&owned_path))
        .await
        .map_err(|join_err| ModelFetchError::Io(std::io::Error::other(join_err)))??;
    Ok(actual.eq_ignore_ascii_case(expected_sha256_hex))
}

async fn stream_to_file(
    response: reqwest::Response,
    part_path: &Path,
) -> Result<String, ModelFetchError> {
    let mut file = tokio::fs::File::create(part_path).await?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Blocking full-file hash — run inside `spawn_blocking`, never on the async
/// executor thread (model files run into the gigabytes).
fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::Router;

    use super::*;

    const BODY: &[u8] = b"streamdubber local stt model bytes (test fixture)";

    fn body_sha256_hex() -> String {
        let mut hasher = Sha256::new();
        hasher.update(BODY);
        hex::encode(hasher.finalize())
    }

    async fn spawn_server() -> (SocketAddr, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let counted_hits = Arc::clone(&hits);

        let app = Router::new()
            .route("/ok.bin", get(|| async { BODY }))
            .route(
                "/counted.bin",
                get(move || {
                    let hits = Arc::clone(&counted_hits);
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        BODY
                    }
                }),
            )
            .route("/missing.bin", get(|| async { StatusCode::NOT_FOUND }));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        (addr, hits)
    }

    fn scratch_path(label: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "streamdubber-model-fetch-test-{label}-{unique}.bin"
        ))
    }

    #[tokio::test]
    async fn downloads_and_verifies_fresh_file() {
        let (addr, _hits) = spawn_server().await;
        let http = reqwest::Client::new();
        let dest = scratch_path("fresh");

        let result = ensure_model(
            &http,
            &format!("http://{addr}/ok.bin"),
            &body_sha256_hex(),
            &dest,
        )
        .await
        .expect("download");

        assert_eq!(result, dest);
        assert_eq!(tokio::fs::read(&dest).await.expect("read"), BODY);
        assert!(!part_path_for(&dest).exists(), ".part temizlenmeli");
        let _ = tokio::fs::remove_file(&dest).await;
    }

    #[tokio::test]
    async fn checksum_mismatch_is_rejected_and_cleans_up() {
        let (addr, _hits) = spawn_server().await;
        let http = reqwest::Client::new();
        let dest = scratch_path("mismatch");

        let err = ensure_model(
            &http,
            &format!("http://{addr}/ok.bin"),
            &"0".repeat(64),
            &dest,
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ModelFetchError::ChecksumMismatch { .. }));
        assert!(!dest.exists(), "kötü indirme dest'e taşınmamalı");
        assert!(!part_path_for(&dest).exists(), ".part temizlenmeli");
    }

    #[tokio::test]
    async fn http_error_status_is_reported() {
        let (addr, _hits) = spawn_server().await;
        let http = reqwest::Client::new();
        let dest = scratch_path("404");

        let err = ensure_model(
            &http,
            &format!("http://{addr}/missing.bin"),
            &body_sha256_hex(),
            &dest,
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ModelFetchError::Status(StatusCode::NOT_FOUND)));
    }

    #[tokio::test]
    async fn already_correct_file_skips_download() {
        let (addr, hits) = spawn_server().await;
        let http = reqwest::Client::new();
        let dest = scratch_path("cached");
        tokio::fs::write(&dest, BODY).await.expect("seed file");

        let result = ensure_model(
            &http,
            &format!("http://{addr}/counted.bin"),
            &body_sha256_hex(),
            &dest,
        )
        .await
        .expect("should short-circuit");

        assert_eq!(result, dest);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            0,
            "zaten doğru dosya indirilmemeli"
        );
        let _ = tokio::fs::remove_file(&dest).await;
    }

    #[tokio::test]
    async fn stale_file_at_dest_is_redownloaded() {
        let (addr, hits) = spawn_server().await;
        let http = reqwest::Client::new();
        let dest = scratch_path("stale");
        tokio::fs::write(&dest, b"old garbage bytes")
            .await
            .expect("seed stale file");

        let result = ensure_model(
            &http,
            &format!("http://{addr}/counted.bin"),
            &body_sha256_hex(),
            &dest,
        )
        .await
        .expect("redownload");

        assert_eq!(result, dest);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(tokio::fs::read(&dest).await.expect("read"), BODY);
        let _ = tokio::fs::remove_file(&dest).await;
    }
}
