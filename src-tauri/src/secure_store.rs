use crate::error::DiscoveryError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub trait SecretStore: Send + Sync {
    fn save_refresh_token(&self, account_id: &str, token: &str) -> Result<(), DiscoveryError>;
    fn read_refresh_token(&self, account_id: &str) -> Result<Option<String>, DiscoveryError>;
}

#[derive(Clone, Default)]
pub struct InMemorySecretStore {
    tokens: Arc<Mutex<HashMap<String, String>>>,
}

impl SecretStore for InMemorySecretStore {
    fn save_refresh_token(&self, account_id: &str, token: &str) -> Result<(), DiscoveryError> {
        self.tokens
            .lock()
            .map_err(|_| DiscoveryError::Storage("token store lock poisoned".into()))?
            .insert(account_id.to_string(), token.to_string());
        Ok(())
    }

    fn read_refresh_token(&self, account_id: &str) -> Result<Option<String>, DiscoveryError> {
        Ok(self
            .tokens
            .lock()
            .map_err(|_| DiscoveryError::Storage("token store lock poisoned".into()))?
            .get(account_id)
            .cloned())
    }
}
