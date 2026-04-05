use crate::domain::AppSnapshot;
use crate::error::DiscoveryError;

pub trait LocalStore: Send + Sync {
    fn load_snapshot(&self) -> Result<AppSnapshot, DiscoveryError>;
    fn save_snapshot(&self, snapshot: AppSnapshot) -> Result<(), DiscoveryError>;
}

#[derive(Clone)]
pub struct InMemoryStore {
    snapshot: std::sync::Arc<std::sync::Mutex<AppSnapshot>>,
}

impl InMemoryStore {
    pub fn new(snapshot: AppSnapshot) -> Self {
        Self {
            snapshot: std::sync::Arc::new(std::sync::Mutex::new(snapshot)),
        }
    }
}

impl LocalStore for InMemoryStore {
    fn load_snapshot(&self) -> Result<AppSnapshot, DiscoveryError> {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| DiscoveryError::Storage("snapshot lock poisoned".into()))
    }

    fn save_snapshot(&self, snapshot: AppSnapshot) -> Result<(), DiscoveryError> {
        let mut guard = self
            .snapshot
            .lock()
            .map_err(|_| DiscoveryError::Storage("snapshot lock poisoned".into()))?;
        *guard = snapshot;
        Ok(())
    }
}
