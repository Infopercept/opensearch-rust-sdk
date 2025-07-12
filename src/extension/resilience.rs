use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use crate::extension::ExtensionError;

#[derive(Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f32,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            exponential_base: 2.0,
            jitter: true,
        }
    }
}

pub async fn retry_with_policy<F, Fut, T>(
    policy: &RetryPolicy,
    mut operation: F,
) -> Result<T, ExtensionError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ExtensionError>>,
{
    let mut attempt = 0;
    let mut delay = policy.initial_delay;
    
    loop {
        attempt += 1;
        
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt >= policy.max_attempts => {
                return Err(ExtensionError::unknown(
                    format!("Operation failed after {} attempts: {}", policy.max_attempts, e)
                ));
            }
            Err(_) => {
                let jittered_delay = if policy.jitter {
                    let jitter = rand::random::<f32>() * 0.3;
                    delay.mul_f32(1.0 + jitter)
                } else {
                    delay
                };
                
                sleep(jittered_delay).await;
                
                delay = Duration::from_secs_f32(
                    (delay.as_secs_f32() * policy.exponential_base).min(policy.max_delay.as_secs_f32())
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitBreakerState>>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
}

struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, success_threshold: u32, timeout: Duration) -> Self {
        CircuitBreaker {
            state: Arc::new(Mutex::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
            })),
            failure_threshold,
            success_threshold,
            timeout,
        }
    }
    
    pub async fn call<F, Fut, T>(&self, operation: F) -> Result<T, ExtensionError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, ExtensionError>>,
    {
        let mut state = self.state.lock().await;
        
        match state.state {
            CircuitState::Open => {
                if let Some(last_failure) = state.last_failure_time {
                    if last_failure.elapsed() >= self.timeout {
                        state.state = CircuitState::HalfOpen;
                        state.failure_count = 0;
                        state.success_count = 0;
                    } else {
                        return Err(ExtensionError::unknown("Circuit breaker is open"));
                    }
                } else {
                    return Err(ExtensionError::unknown("Circuit breaker is open"));
                }
            }
            _ => {}
        }
        
        drop(state);
        
        match operation().await {
            Ok(result) => {
                let mut state = self.state.lock().await;
                match state.state {
                    CircuitState::HalfOpen => {
                        state.success_count += 1;
                        if state.success_count >= self.success_threshold {
                            state.state = CircuitState::Closed;
                            state.failure_count = 0;
                        }
                    }
                    CircuitState::Closed => {
                        state.failure_count = 0;
                    }
                    _ => {}
                }
                Ok(result)
            }
            Err(e) => {
                let mut state = self.state.lock().await;
                state.failure_count += 1;
                state.last_failure_time = Some(Instant::now());
                
                match state.state {
                    CircuitState::Closed => {
                        if state.failure_count >= self.failure_threshold {
                            state.state = CircuitState::Open;
                        }
                    }
                    CircuitState::HalfOpen => {
                        state.state = CircuitState::Open;
                    }
                    _ => {}
                }
                
                Err(e)
            }
        }
    }
    
    pub async fn get_state(&self) -> CircuitState {
        self.state.lock().await.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_retry_success() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            exponential_base: 2.0,
            jitter: false,
        };
        
        let mut attempt_count = 0;
        let result = retry_with_policy(&policy, || {
            attempt_count += 1;
            async move {
                if attempt_count < 3 {
                    Err(ExtensionError::unknown("temporary failure"))
                } else {
                    Ok("success")
                }
            }
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }
    
    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(2, 2, Duration::from_millis(100));
        
        assert_eq!(cb.get_state().await, CircuitState::Closed);
        
        let _ = cb.call(|| async { Err::<(), _>(ExtensionError::unknown("fail")) }).await;
        let _ = cb.call(|| async { Err::<(), _>(ExtensionError::unknown("fail")) }).await;
        
        assert_eq!(cb.get_state().await, CircuitState::Open);
        
        let result = cb.call(|| async { Ok("should fail") }).await;
        assert!(result.is_err());
        
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        let _ = cb.call(|| async { Ok("success") }).await;
        let _ = cb.call(|| async { Ok("success") }).await;
        
        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }
}