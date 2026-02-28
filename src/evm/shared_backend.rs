//! SharedBackend — a thread-safe, lazily-populated backend for REVM.
//!
//! Spawns a background tokio task that owns the RPC connection (AlloyDB).
//! Multiple concurrent simulations share the same cached state by creating
//! `CacheDB<SharedBackend>` instances, each with isolated writes.
//!
//! ```text
//!                      Background Task
//!                  ┌──────────────────────┐
//!                  │  AlloyDB (RPC)       │
//!                  │  + CacheDB (cache)   │
//!                  └──────────▲───────────┘
//!                             │ mpsc channel
//!          ┌─────────────────┼──────────────────┐
//!          │                 │                   │
//!   CacheDB<SharedBackend>  CacheDB<...>   CacheDB<...>
//!   (isolated writes)       (isolated)     (isolated)
//! ```

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

use alloy::eips::BlockId;
use alloy::network::Ethereum;
use alloy::providers::Provider;
use alloy_primitives::{Address, B256, U256};
use revm::database::{AlloyDB, CacheDB, Database, DatabaseRef};
use revm::database_interface::WrapDatabaseAsync;
use revm::state::{AccountInfo, Bytecode};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned by SharedBackend operations.
#[derive(Debug, Clone)]
pub struct BackendError(pub String);

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BackendError: {}", self.0)
    }
}

impl std::error::Error for BackendError {}

/// Marker trait required by REVM's Database.
impl revm::database_interface::DBErrorMarker for BackendError {}

impl BackendError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

// ---------------------------------------------------------------------------
// Request/Response types
// ---------------------------------------------------------------------------

enum BackendRequest {
    Basic {
        address: Address,
        tx: oneshot::Sender<Result<Option<AccountInfo>, BackendError>>,
    },
    Storage {
        address: Address,
        index: U256,
        tx: oneshot::Sender<Result<U256, BackendError>>,
    },
    BlockHash {
        number: u64,
        tx: oneshot::Sender<Result<B256, BackendError>>,
    },
    CodeByHash {
        code_hash: B256,
        tx: oneshot::Sender<Result<Bytecode, BackendError>>,
    },
}

// ---------------------------------------------------------------------------
// SharedBackend
// ---------------------------------------------------------------------------

/// A cloneable handle to the shared EVM state backend.
///
/// Create per-simulation databases with [`new_cache_db()`](Self::new_cache_db):
/// each `CacheDB<SharedBackend>` gets isolated writes on top of the shared
/// read cache, so one simulation's state changes never leak into another.
#[derive(Clone)]
pub struct SharedBackend {
    request_tx: mpsc::Sender<BackendRequest>,
    // Lightweight stats — useful for debugging RPC usage
    rpc_fetches: std::sync::Arc<AtomicUsize>,
}

impl SharedBackend {
    /// Spawn the background task and return a handle.
    ///
    /// The task owns the `AlloyDB` provider and serves all RPC requests.
    /// No accounts are populated on startup — state is fetched lazily from
    /// the RPC on first access and cached for subsequent reads.
    pub fn spawn<P>(provider: P, block_id: BlockId) -> Self
    where
        P: Provider + Clone + 'static,
    {
        let (request_tx, request_rx) = mpsc::channel::<BackendRequest>(512);
        let rpc_fetches = std::sync::Arc::new(AtomicUsize::new(0));
        let fetches = rpc_fetches.clone();

        tokio::spawn(async move {
            Self::backend_task(provider, block_id, request_rx, fetches).await;
        });

        Self {
            request_tx,
            rpc_fetches,
        }
    }

    /// Create a new `CacheDB` with this backend as the underlying store.
    ///
    /// The returned database has its own isolated write layer: EVM execution
    /// within one `CacheDB` does not affect any other.
    pub fn new_cache_db(&self) -> CacheDB<SharedBackend> {
        CacheDB::new(self.clone())
    }

    /// Total number of RPC fetches (cache misses) since creation.
    pub fn rpc_fetch_count(&self) -> usize {
        self.rpc_fetches.load(Ordering::Relaxed)
    }

    // -- background task ----------------------------------------------------

    async fn backend_task<P>(
        provider: P,
        block_id: BlockId,
        mut rx: mpsc::Receiver<BackendRequest>,
        fetches: std::sync::Arc<AtomicUsize>,
    ) where
        P: Provider + Clone + 'static,
    {
        // CacheDB<WrapDatabaseAsync<AlloyDB>> handles the async→sync bridge
        // and caches every value fetched from RPC.
        let alloy_db = match WrapDatabaseAsync::new(AlloyDB::new(provider, block_id)) {
            Some(db) => db,
            None => {
                warn!("failed to create AlloyDB — backend task exiting");
                return;
            }
        };
        let mut db: CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, P>>> = CacheDB::new(alloy_db);

        debug!("shared backend task started");

        while let Some(req) = rx.recv().await {
            match req {
                BackendRequest::Basic { address, tx } => {
                    fetches.fetch_add(1, Ordering::Relaxed);
                    let result = db
                        .basic(address)
                        .map_err(|e| BackendError::new(format!("{e:?}")));
                    let _ = tx.send(result);
                }
                BackendRequest::Storage { address, index, tx } => {
                    fetches.fetch_add(1, Ordering::Relaxed);
                    let result = db
                        .storage(address, index)
                        .map_err(|e| BackendError::new(format!("{e:?}")));
                    let _ = tx.send(result);
                }
                BackendRequest::BlockHash { number, tx } => {
                    fetches.fetch_add(1, Ordering::Relaxed);
                    let result = db
                        .block_hash(number)
                        .map_err(|e| BackendError::new(format!("{e:?}")));
                    let _ = tx.send(result);
                }
                BackendRequest::CodeByHash { code_hash, tx } => {
                    fetches.fetch_add(1, Ordering::Relaxed);
                    let result = db
                        .code_by_hash(code_hash)
                        .map_err(|e| BackendError::new(format!("{e:?}")));
                    let _ = tx.send(result);
                }
            }
        }

        debug!("shared backend task stopped");
    }

    // -- internal request helpers -------------------------------------------

    fn send_and_recv<T>(
        &self,
        req: BackendRequest,
        rx: oneshot::Receiver<Result<T, BackendError>>,
    ) -> Result<T, BackendError> {
        tokio::task::block_in_place(|| {
            self.request_tx
                .blocking_send(req)
                .map_err(|_| BackendError::new("backend task stopped"))?;
            rx.blocking_recv()
                .map_err(|_| BackendError::new("response channel dropped"))?
        })
    }
}

// ---------------------------------------------------------------------------
// REVM Database trait implementations
// ---------------------------------------------------------------------------

impl DatabaseRef for SharedBackend {
    type Error = BackendError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.send_and_recv(BackendRequest::Basic { address, tx }, rx)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.send_and_recv(BackendRequest::CodeByHash { code_hash, tx }, rx)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.send_and_recv(BackendRequest::Storage { address, index, tx }, rx)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        let (tx, rx) = oneshot::channel();
        self.send_and_recv(BackendRequest::BlockHash { number, tx }, rx)
    }
}

impl Database for SharedBackend {
    type Error = BackendError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.basic_ref(address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.code_by_hash_ref(code_hash)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.storage_ref(address, index)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        self.block_hash_ref(number)
    }
}
