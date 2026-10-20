use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session};

pub fn is_torrent(input: &str) -> bool {
    let input = input.to_lowercase();
    input.starts_with("magnet:") || input.split(['?', '#']).next().unwrap_or("").ends_with(".torrent")
}

pub fn download(input: &str, dir: &str, on_progress: impl Fn(u64, Option<u64>)) -> Result<u64, String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(run(input, dir, on_progress))
}

async fn run(input: &str, dir: &str, on_progress: impl Fn(u64, Option<u64>)) -> Result<u64, String> {
    let session = Session::new(PathBuf::from(dir)).await.map_err(|e| e.to_string())?;
    let handle = add_torrent(&session, input).await?;
    let result = wait_with_progress(&handle, on_progress).await;
    session.stop().await;
    result
}

async fn add_torrent(session: &Arc<Session>, input: &str) -> Result<Arc<ManagedTorrent>, String> {
    let torrent = AddTorrent::from_cli_argument(input).map_err(|e| e.to_string())?;
    let options = AddTorrentOptions { overwrite: true, ..Default::default() };
    session
        .add_torrent(torrent, Some(options))
        .await
        .map_err(|e| e.to_string())?
        .into_handle()
        .ok_or_else(|| "Torrent konnte nicht hinzugefügt werden".to_string())
}

async fn wait_with_progress(
    handle: &ManagedTorrent,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<u64, String> {
    loop {
        let stats = handle.stats();
        if let Some(e) = stats.error {
            return Err(e);
        }
        let total = (stats.total_bytes > 0).then_some(stats.total_bytes);
        on_progress(stats.progress_bytes, total);
        if stats.finished {
            return Ok(stats.total_bytes);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
