use std::path::Path;
use std::sync::Arc;
use aurafs_core::models::{ConflictRecord, DeviceInfo, FileMetadata, SyncOperation, SyncStatus};
use redb::{Database, ReadableTable, TableDefinition};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

/// High-performance, memory-optimized cache error type.
///
/// Large upstream error types are explicitly boxed on the heap to keep `Result<T, CacheError>`
/// stack frames minimal (< 24 bytes), preventing stack bloat in high-throughput hot paths.
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Database error: {0}")]
    Db(Box<redb::Error>),
    #[error("Database creation error: {0}")]
    Database(Box<redb::DatabaseError>),
    #[error("Database transaction error: {0}")]
    Transaction(Box<redb::TransactionError>),
    #[error("Table error: {0}")]
    Table(Box<redb::TableError>),
    #[error("Commit error: {0}")]
    Commit(Box<redb::CommitError>),
    #[error("Serialization error: {0}")]
    Serialization(Box<bincode::ErrorKind>),
    #[error("Storage error: {0}")]
    Storage(Box<redb::StorageError>),
    #[error("Actor communication channel closed")]
    ChannelClosed,
    #[allow(dead_code)]
    #[error("Item not found: {0}")]
    NotFound(String),
}

impl From<redb::Error> for CacheError {
    fn from(e: redb::Error) -> Self {
        CacheError::Db(Box::new(e))
    }
}

impl From<redb::DatabaseError> for CacheError {
    fn from(e: redb::DatabaseError) -> Self {
        CacheError::Database(Box::new(e))
    }
}

impl From<redb::TransactionError> for CacheError {
    fn from(e: redb::TransactionError) -> Self {
        CacheError::Transaction(Box::new(e))
    }
}

impl From<redb::TableError> for CacheError {
    fn from(e: redb::TableError) -> Self {
        CacheError::Table(Box::new(e))
    }
}

impl From<redb::CommitError> for CacheError {
    fn from(e: redb::CommitError) -> Self {
        CacheError::Commit(Box::new(e))
    }
}

impl From<Box<bincode::ErrorKind>> for CacheError {
    fn from(e: Box<bincode::ErrorKind>) -> Self {
        CacheError::Serialization(e)
    }
}

impl From<redb::StorageError> for CacheError {
    fn from(e: redb::StorageError) -> Self {
        CacheError::Storage(Box::new(e))
    }
}

// Table Definitions for Pure-Rust Embedded Storage (redb)
const TABLE_FILE_META: TableDefinition<&str, &[u8]> = TableDefinition::new("file_metadata");
const TABLE_SYNC_QUEUE: TableDefinition<u64, &[u8]> = TableDefinition::new("sync_queue");
const TABLE_DEVICE_STATE: TableDefinition<&str, &[u8]> = TableDefinition::new("device_state");
const TABLE_TAGS: TableDefinition<&str, &str> = TableDefinition::new("tags");
const TABLE_CONFLICTS: TableDefinition<&str, &[u8]> = TableDefinition::new("conflicts");

/// Write commands executed exclusively by the dedicated Database Writer Actor task.
pub enum DbWriteCommand {
    UpsertFileMetadata {
        metadata: FileMetadata,
        responder: oneshot::Sender<Result<(), CacheError>>,
    },
    UpdateSyncStatus {
        path: String,
        status: SyncStatus,
        responder: oneshot::Sender<Result<(), CacheError>>,
    },
    EnqueueSyncOperation {
        operation: SyncOperation,
        responder: oneshot::Sender<Result<u64, CacheError>>,
    },
    #[allow(dead_code)]
    DeleteSyncOperation {
        sequence_id: u64,
        responder: oneshot::Sender<Result<(), CacheError>>,
    },
    SetTag {
        path: String,
        key: String,
        value: String,
        responder: oneshot::Sender<Result<(), CacheError>>,
    },
    #[allow(dead_code)]
    RecordConflict {
        conflict: ConflictRecord,
        responder: oneshot::Sender<Result<(), CacheError>>,
    },
    SaveDeviceState {
        info: DeviceInfo,
        responder: oneshot::Sender<Result<(), CacheError>>,
    },
}

/// Thread-safe client handle to the AuraFS L1 Cache.
///
/// Reads execute concurrently and non-blockingly via `redb::begin_read()`.
/// Writes are serialized through an actor task to prevent lock contention with the Axum API.
#[derive(Clone)]
pub struct CacheManager {
    db: Arc<Database>,
    write_tx: mpsc::Sender<DbWriteCommand>,
}

impl CacheManager {
    /// Initialize the local database file, create tables if they do not exist, and spawn the writer actor.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let db = Arc::new(Database::create(path)?);

        // Initialize table schemas using a write transaction
        {
            let write_txn = db.begin_write()?;
            let _ = write_txn.open_table(TABLE_FILE_META)?;
            let _ = write_txn.open_table(TABLE_SYNC_QUEUE)?;
            let _ = write_txn.open_table(TABLE_DEVICE_STATE)?;
            let _ = write_txn.open_table(TABLE_TAGS)?;
            let _ = write_txn.open_table(TABLE_CONFLICTS)?;
            write_txn.commit()?;
        }

        let (write_tx, mut write_rx) = mpsc::channel::<DbWriteCommand>(1024);
        let actor_db = Arc::clone(&db);

        // Spawn Dedicated Writer Actor Task
        tokio::spawn(async move {
            while let Some(cmd) = write_rx.recv().await {
                match cmd {
                    DbWriteCommand::UpsertFileMetadata { metadata, responder } => {
                        let res = Self::handle_upsert_file(&actor_db, metadata);
                        let _ = responder.send(res);
                    }
                    DbWriteCommand::UpdateSyncStatus { path, status, responder } => {
                        let res = Self::handle_update_status(&actor_db, &path, status);
                        let _ = responder.send(res);
                    }
                    DbWriteCommand::EnqueueSyncOperation { operation, responder } => {
                        let res = Self::handle_enqueue(&actor_db, operation);
                        let _ = responder.send(res);
                    }
                    DbWriteCommand::DeleteSyncOperation { sequence_id, responder } => {
                        let res = Self::handle_delete_op(&actor_db, sequence_id);
                        let _ = responder.send(res);
                    }
                    DbWriteCommand::SetTag { path, key, value, responder } => {
                        let res = Self::handle_set_tag(&actor_db, &path, &key, &value);
                        let _ = responder.send(res);
                    }
                    DbWriteCommand::RecordConflict { conflict, responder } => {
                        let res = Self::handle_record_conflict(&actor_db, conflict);
                        let _ = responder.send(res);
                    }
                    DbWriteCommand::SaveDeviceState { info, responder } => {
                        let res = Self::handle_save_device(&actor_db, info);
                        let _ = responder.send(res);
                    }
                }
            }
        });

        Ok(Self { db, write_tx })
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Actor Internal Write Handlers
    // -----------------------------------------------------------------------------------------------------------------

    fn handle_upsert_file(db: &Database, metadata: FileMetadata) -> Result<(), CacheError> {
        let bytes = bincode::serialize(&metadata)?;
        let write_txn = db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE_FILE_META)?;
            table.insert(metadata.relative_path.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn handle_update_status(db: &Database, path: &str, status: SyncStatus) -> Result<(), CacheError> {
        let write_txn = db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE_FILE_META)?;
            let existing_bytes = table.get(path)?.map(|guard| guard.value().to_vec());
            if let Some(raw) = existing_bytes {
                let mut meta: FileMetadata = bincode::deserialize(&raw)?;
                meta.sync_status = status;
                let new_bytes = bincode::serialize(&meta)?;
                table.insert(path, new_bytes.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    fn handle_enqueue(db: &Database, mut op: SyncOperation) -> Result<u64, CacheError> {
        let write_txn = db.begin_write()?;
        let seq_id = {
            let mut table = write_txn.open_table(TABLE_SYNC_QUEUE)?;
            // Compute next sequence ID monotonically
            let last_id = table.iter()?.next_back().map(|item| item.map(|(k, _)| k.value())).transpose()?.unwrap_or(0);
            let next_id = last_id + 1;
            op.sequence_id = next_id;
            let bytes = bincode::serialize(&op)?;
            table.insert(next_id, bytes.as_slice())?;
            next_id
        };
        write_txn.commit()?;
        Ok(seq_id)
    }

    fn handle_delete_op(db: &Database, sequence_id: u64) -> Result<(), CacheError> {
        let write_txn = db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE_SYNC_QUEUE)?;
            table.remove(sequence_id)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn handle_set_tag(db: &Database, path: &str, key: &str, value: &str) -> Result<(), CacheError> {
        let composite_key = format!("{}::{}", path, key);
        let write_txn = db.begin_write()?;
        {
            let mut tags_table = write_txn.open_table(TABLE_TAGS)?;
            tags_table.insert(composite_key.as_str(), value)?;

            let mut meta_table = write_txn.open_table(TABLE_FILE_META)?;
            let existing_bytes = meta_table.get(path)?.map(|guard| guard.value().to_vec());
            if let Some(raw) = existing_bytes {
                let mut meta: FileMetadata = bincode::deserialize(&raw)?;
                meta.tags.insert(key.to_string(), value.to_string());
                let bytes = bincode::serialize(&meta)?;
                meta_table.insert(path, bytes.as_slice())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    fn handle_record_conflict(db: &Database, conflict: ConflictRecord) -> Result<(), CacheError> {
        let bytes = bincode::serialize(&conflict)?;
        let write_txn = db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE_CONFLICTS)?;
            table.insert(conflict.conflict_id.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn handle_save_device(db: &Database, info: DeviceInfo) -> Result<(), CacheError> {
        let bytes = bincode::serialize(&info)?;
        let write_txn = db.begin_write()?;
        {
            let mut table = write_txn.open_table(TABLE_DEVICE_STATE)?;
            table.insert("self", bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Public Asynchronous Write API (Communicates with Actor)
    // -----------------------------------------------------------------------------------------------------------------

    pub async fn upsert_file_metadata(&self, metadata: FileMetadata) -> Result<(), CacheError> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send(DbWriteCommand::UpsertFileMetadata { metadata, responder: tx })
            .await
            .map_err(|_| CacheError::ChannelClosed)?;
        rx.await.map_err(|_| CacheError::ChannelClosed)?
    }

    pub async fn update_sync_status(&self, path: impl Into<String>, status: SyncStatus) -> Result<(), CacheError> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send(DbWriteCommand::UpdateSyncStatus { path: path.into(), status, responder: tx })
            .await
            .map_err(|_| CacheError::ChannelClosed)?;
        rx.await.map_err(|_| CacheError::ChannelClosed)?
    }

    pub async fn enqueue_sync_operation(&self, operation: SyncOperation) -> Result<u64, CacheError> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send(DbWriteCommand::EnqueueSyncOperation { operation, responder: tx })
            .await
            .map_err(|_| CacheError::ChannelClosed)?;
        rx.await.map_err(|_| CacheError::ChannelClosed)?
    }

    #[allow(dead_code)]
    pub async fn delete_sync_operation(&self, sequence_id: u64) -> Result<(), CacheError> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send(DbWriteCommand::DeleteSyncOperation { sequence_id, responder: tx })
            .await
            .map_err(|_| CacheError::ChannelClosed)?;
        rx.await.map_err(|_| CacheError::ChannelClosed)?
    }

    pub async fn set_file_tag(&self, path: impl Into<String>, key: impl Into<String>, value: impl Into<String>) -> Result<(), CacheError> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send(DbWriteCommand::SetTag {
                path: path.into(),
                key: key.into(),
                value: value.into(),
                responder: tx,
            })
            .await
            .map_err(|_| CacheError::ChannelClosed)?;
        rx.await.map_err(|_| CacheError::ChannelClosed)?
    }

    #[allow(dead_code)]
    pub async fn record_conflict(&self, conflict: ConflictRecord) -> Result<(), CacheError> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send(DbWriteCommand::RecordConflict { conflict, responder: tx })
            .await
            .map_err(|_| CacheError::ChannelClosed)?;
        rx.await.map_err(|_| CacheError::ChannelClosed)?
    }

    pub async fn save_device_state(&self, info: DeviceInfo) -> Result<(), CacheError> {
        let (tx, rx) = oneshot::channel();
        self.write_tx
            .send(DbWriteCommand::SaveDeviceState { info, responder: tx })
            .await
            .map_err(|_| CacheError::ChannelClosed)?;
        rx.await.map_err(|_| CacheError::ChannelClosed)?
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Public Concurrent Read API (Non-blocking MVCC Snapshots)
    // -----------------------------------------------------------------------------------------------------------------

    pub fn get_file_metadata(&self, path: &str) -> Result<Option<FileMetadata>, CacheError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE_FILE_META)?;
        if let Some(guard) = table.get(path)? {
            let meta: FileMetadata = bincode::deserialize(guard.value())?;
            Ok(Some(meta))
        } else {
            Ok(None)
        }
    }

    pub fn list_all_files(&self) -> Result<Vec<FileMetadata>, CacheError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE_FILE_META)?;
        let mut results = Vec::new();

        for item in table.iter()? {
            let (_, val) = item?;
            let meta: FileMetadata = bincode::deserialize(val.value())?;
            results.push(meta);
        }

        Ok(results)
    }

    pub fn get_pending_sync_operations(&self) -> Result<Vec<SyncOperation>, CacheError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE_SYNC_QUEUE)?;
        let mut operations = Vec::new();

        for item in table.iter()? {
            let (_, val) = item?;
            let op: SyncOperation = bincode::deserialize(val.value())?;
            operations.push(op);
        }

        Ok(operations)
    }

    pub fn find_files_by_tag(&self, key: &str, value: &str) -> Result<Vec<FileMetadata>, CacheError> {
        let all_files = self.list_all_files()?;
        let matched = all_files
            .into_iter()
            .filter(|f| f.tags.get(key).map(|v| v == value).unwrap_or(false))
            .collect();
        Ok(matched)
    }

    pub fn get_device_state(&self) -> Result<Option<DeviceInfo>, CacheError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(TABLE_DEVICE_STATE)?;
        if let Some(guard) = table.get("self")? {
            let info: DeviceInfo = bincode::deserialize(guard.value())?;
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_cache_crud_and_actor_concurrency() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("test_aura.redb");

        let cache = CacheManager::new(&db_path).expect("Failed to init cache");

        let meta = FileMetadata::new(
            "docs/architecture.md",
            1024,
            "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            Utc::now(),
            Utc::now(),
        );

        // 1. Insert file metadata
        cache.upsert_file_metadata(meta.clone()).await.expect("upsert failed");

        // 2. Read file metadata
        let retrieved = cache.get_file_metadata("docs/architecture.md").expect("read failed");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().size_bytes, 1024);

        // 3. Set and query tags
        cache
            .set_file_tag("docs/architecture.md", "category", "documentation")
            .await
            .expect("set tag failed");

        let tagged_files = cache.find_files_by_tag("category", "documentation").expect("tag query failed");
        assert_eq!(tagged_files.len(), 1);
        assert_eq!(tagged_files[0].relative_path, "docs/architecture.md");

        // 4. Queue operations
        let op = SyncOperation::new_put(0, "docs/architecture.md", meta.blake3_hash.clone(), 1024);
        let seq = cache.enqueue_sync_operation(op).await.expect("enqueue failed");
        assert_eq!(seq, 1);

        let pending = cache.get_pending_sync_operations().expect("get pending failed");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].sequence_id, 1);

        // 5. Delete operation
        cache.delete_sync_operation(1).await.expect("delete op failed");
        let pending_after = cache.get_pending_sync_operations().expect("get pending failed");
        assert_eq!(pending_after.len(), 0);
    }
}
