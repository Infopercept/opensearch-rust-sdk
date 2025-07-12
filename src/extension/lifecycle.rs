use std::sync::Arc;
use tokio::sync::RwLock;
use crate::extension::ExtensionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionState {
    Created,
    Initializing,
    Initialized,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl ExtensionState {
    pub fn can_transition_to(&self, next: ExtensionState) -> bool {
        match (*self, next) {
            (ExtensionState::Created, ExtensionState::Initializing) => true,
            (ExtensionState::Initializing, ExtensionState::Initialized) => true,
            (ExtensionState::Initializing, ExtensionState::Failed) => true,
            (ExtensionState::Initialized, ExtensionState::Running) => true,
            (ExtensionState::Initialized, ExtensionState::Stopping) => true,
            (ExtensionState::Running, ExtensionState::Stopping) => true,
            (ExtensionState::Stopping, ExtensionState::Stopped) => true,
            (ExtensionState::Stopping, ExtensionState::Failed) => true,
            (_, ExtensionState::Failed) => true,
            _ => false,
        }
    }
    
    pub fn is_terminal(&self) -> bool {
        matches!(self, ExtensionState::Stopped | ExtensionState::Failed)
    }
    
    pub fn is_running(&self) -> bool {
        matches!(self, ExtensionState::Running)
    }
}

pub struct LifecycleManager {
    state: Arc<RwLock<ExtensionState>>,
    state_listeners: Arc<RwLock<Vec<Box<dyn StateListener>>>>,
}

#[async_trait::async_trait]
pub trait StateListener: Send + Sync {
    async fn on_state_change(&self, old_state: ExtensionState, new_state: ExtensionState);
}

impl LifecycleManager {
    pub fn new() -> Self {
        LifecycleManager {
            state: Arc::new(RwLock::new(ExtensionState::Created)),
            state_listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub async fn current_state(&self) -> ExtensionState {
        *self.state.read().await
    }
    
    pub async fn transition_to(&self, new_state: ExtensionState) -> Result<(), ExtensionError> {
        let mut current = self.state.write().await;
        
        if !current.can_transition_to(new_state) {
            return Err(ExtensionError::initialization(
                format!("Invalid state transition from {:?} to {:?}", *current, new_state)
            ));
        }
        
        let old_state = *current;
        *current = new_state;
        drop(current);
        
        self.notify_listeners(old_state, new_state).await;
        
        Ok(())
    }
    
    pub async fn add_listener(&self, listener: Box<dyn StateListener>) {
        let mut listeners = self.state_listeners.write().await;
        listeners.push(listener);
    }
    
    async fn notify_listeners(&self, old_state: ExtensionState, new_state: ExtensionState) {
        let listeners = self.state_listeners.read().await;
        for listener in listeners.iter() {
            listener.on_state_change(old_state, new_state).await;
        }
    }
    
    pub async fn is_running(&self) -> bool {
        self.current_state().await.is_running()
    }
    
    pub async fn is_terminal(&self) -> bool {
        self.current_state().await.is_terminal()
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct LoggingStateListener;

#[async_trait::async_trait]
impl StateListener for LoggingStateListener {
    async fn on_state_change(&self, old_state: ExtensionState, new_state: ExtensionState) {
        tracing::info!(
            "Extension state changed from {:?} to {:?}",
            old_state,
            new_state
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_transitions() {
        assert!(ExtensionState::Created.can_transition_to(ExtensionState::Initializing));
        assert!(ExtensionState::Initializing.can_transition_to(ExtensionState::Initialized));
        assert!(ExtensionState::Initialized.can_transition_to(ExtensionState::Running));
        assert!(ExtensionState::Running.can_transition_to(ExtensionState::Stopping));
        assert!(ExtensionState::Stopping.can_transition_to(ExtensionState::Stopped));
        
        assert!(!ExtensionState::Created.can_transition_to(ExtensionState::Running));
        assert!(!ExtensionState::Stopped.can_transition_to(ExtensionState::Running));
    }
    
    #[tokio::test]
    async fn test_lifecycle_manager() {
        let manager = LifecycleManager::new();
        
        assert_eq!(manager.current_state().await, ExtensionState::Created);
        
        manager.transition_to(ExtensionState::Initializing).await.unwrap();
        assert_eq!(manager.current_state().await, ExtensionState::Initializing);
        
        manager.transition_to(ExtensionState::Initialized).await.unwrap();
        assert_eq!(manager.current_state().await, ExtensionState::Initialized);
        
        let result = manager.transition_to(ExtensionState::Created).await;
        assert!(result.is_err());
    }
}