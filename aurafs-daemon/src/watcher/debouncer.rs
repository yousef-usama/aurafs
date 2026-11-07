use aurafs_core::hashing::hash_file_sync;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{mpsc, Semaphore};

#[derive(Error, Debug)]
pub enum WatcherError {
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Join error: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// A debounced, stabilized file system event ready for synchronization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebouncedFileEvent {
    CreateOrModify {
        relative_path: String,
        absolute_path: PathBuf,
        blake3_hash: String,
        size_bytes: u64,
    },
    Delete {
        relative_path: String,
    },
}

/// File system monitor with event coalescing, write settling detection, and bounded parallel hashing.
pub struct FileWatcherService {
    watch_dir: PathBuf,
    debounce_duration: Duration,
    hashing_semaphore: Arc<Semaphore>,
    event_tx: mpsc::Sender<DebouncedFileEvent>,
}

impl FileWatcherService {
    pub fn new(
        watch_dir: impl Into<PathBuf>,
        debounce_ms: u64,
        max_hashing_concurrency: usize,
        event_tx: mpsc::Sender<DebouncedFileEvent>,
    ) -> Self {
        let watch_dir = watch_dir.into();
        let canonical_dir = watch_dir
            .canonicalize()
            .unwrap_or_else(|_| watch_dir.clone());
        Self {
            watch_dir: canonical_dir,
            debounce_duration: Duration::from_millis(debounce_ms),
            hashing_semaphore: Arc::new(Semaphore::new(max_hashing_concurrency)),
            event_tx,
        }
    }

    /// Start watching the directory and process events until shutdown signal is received.
    pub async fn start(&self) -> Result<(), WatcherError> {
        std::fs::create_dir_all(&self.watch_dir)?;

        let (raw_tx, mut raw_rx) = mpsc::channel::<notify::Result<Event>>(2048);

        // Bridge synchronous notify callbacks into Tokio async channel
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = raw_tx.blocking_send(res);
            },
            notify::Config::default(),
        )?;

        watcher.watch(&self.watch_dir, RecursiveMode::Recursive)?;
        tracing::info!(watch_dir = %self.watch_dir.display(), "Active file system watcher started");

        let watch_dir_clone = self.watch_dir.clone();
        let debounce_dur = self.debounce_duration;
        let semaphore = Arc::clone(&self.hashing_semaphore);
        let out_tx = self.event_tx.clone();

        tokio::spawn(async move {
            let _watcher = watcher; // Keep FSEvents stream alive for lifetime of worker
            let mut pending_paths: HashMap<PathBuf, (Instant, bool)> = HashMap::new();
            let mut tick_interval = tokio::time::interval(Duration::from_millis(50));

            loop {
                tokio::select! {
                    Some(res) = raw_rx.recv() => {
                        if let Ok(event) = res {
                            for path in event.paths {
                                if Self::is_ignored_path(&path) {
                                    continue;
                                }

                                let is_delete = matches!(event.kind, EventKind::Remove(_));
                                pending_paths.insert(path, (Instant::now(), is_delete));
                            }
                        }
                    }
                    _ = tick_interval.tick() => {
                        if pending_paths.is_empty() {
                            continue;
                        }

                        let now = Instant::now();
                        let ready_paths: Vec<(PathBuf, bool)> = pending_paths
                            .iter()
                            .filter(|(_, (last_seen, _))| now.duration_since(*last_seen) >= debounce_dur)
                            .map(|(path, (_, is_del))| (path.clone(), *is_del))
                            .collect();

                        for (path, is_del) in ready_paths {
                            pending_paths.remove(&path);

                            // Calculate clean POSIX relative path from watch directory
                            let relative_path = match path.strip_prefix(&watch_dir_clone) {
                                Ok(rel) => rel.to_string_lossy().to_string(),
                                Err(_) => continue,
                            };

                            if is_del || !path.exists() {
                                let _ = out_tx.send(DebouncedFileEvent::Delete { relative_path }).await;
                                continue;
                            }

                            // Stabilization check: verify file is accessible and metadata is readable
                            if let Ok(metadata) = std::fs::metadata(&path) {
                                if metadata.is_dir() {
                                    continue;
                                }

                                let size_bytes = metadata.len();
                                let path_clone = path.clone();
                                let sem = Arc::clone(&semaphore);
                                let tx = out_tx.clone();

                                // Offload CPU-bound BLAKE3 hashing to blocking thread pool
                                tokio::spawn(async move {
                                    let _permit = match sem.acquire().await {
                                        Ok(p) => p,
                                        Err(_) => return,
                                    };

                                    let hash_res = tokio::task::spawn_blocking(move || {
                                        hash_file_sync(&path_clone)
                                    }).await;

                                    if let Ok(Ok(blake3_hash)) = hash_res {
                                        let _ = tx.send(DebouncedFileEvent::CreateOrModify {
                                            relative_path,
                                            absolute_path: path,
                                            blake3_hash,
                                            size_bytes,
                                        }).await;
                                    }
                                });
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Filter temporary files, editor swap files, hidden directories (.git, .aurafs), and build artifacts (target).
    fn is_ignored_path(path: &Path) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(comp) = component {
                if let Some(s) = comp.to_str() {
                    if s == ".git"
                        || s == ".aurafs"
                        || s == ".github"
                        || s == "target"
                        || s == "node_modules"
                    {
                        return true;
                    }
                }
            }
        }

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if file_name.starts_with('.')
                || file_name.ends_with('~')
                || file_name.ends_with(".tmp")
                || file_name.ends_with(".swp")
                || file_name.ends_with(".redb")
                || file_name.ends_with(".lock")
                || file_name == "Icon\r"
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aurafs_core::hashing::hash_bytes;

    #[tokio::test]
    async fn test_watcher_debouncer_lifecycle() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let watch_dir = temp_dir.path().to_path_buf();

        let (tx, mut rx) = mpsc::channel(32);
        // Fast 150ms debounce for unit test execution
        let watcher = FileWatcherService::new(watch_dir.clone(), 150, 2, tx);
        watcher
            .start()
            .await
            .expect("Failed to start watcher service");

        // Give the OS file watcher thread 50ms to attach
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 1. Create a file
        let file_path = watch_dir.join("sample.txt");
        let file_data = b"AuraFS live filesystem event verification";
        std::fs::write(&file_path, file_data).expect("write failed");

        let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("Timed out waiting for file creation event")
            .expect("Channel closed");

        match event {
            DebouncedFileEvent::CreateOrModify {
                relative_path,
                blake3_hash,
                size_bytes,
                ..
            } => {
                assert_eq!(relative_path, "sample.txt");
                assert_eq!(blake3_hash, hash_bytes(file_data));
                assert_eq!(size_bytes, file_data.len() as u64);
            }
            _ => panic!("Expected CreateOrModify event, got {:?}", event),
        }

        // 2. Delete the file
        std::fs::remove_file(&file_path).expect("remove failed");

        let delete_event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("Timed out waiting for file deletion event")
            .expect("Channel closed");

        match delete_event {
            DebouncedFileEvent::Delete { relative_path } => {
                assert_eq!(relative_path, "sample.txt");
            }
            _ => panic!("Expected Delete event, got {:?}", delete_event),
        }
    }
}
