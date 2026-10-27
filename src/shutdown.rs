//! Process-wide shutdown notification for long-lived sessions.

use tokio::sync::watch;

/// Fires the shutdown notification. Held by the composition root.
#[derive(Debug)]
pub struct ShutdownTrigger(watch::Sender<bool>);

/// Waits for the shutdown notification. Cloned into every session.
#[derive(Debug, Clone)]
pub struct ShutdownSignal(watch::Receiver<bool>);

/// Creates a connected trigger/signal pair.
///
/// # Examples
///
/// ```
/// # tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
/// let (trigger, mut signal) = cisa::shutdown::channel();
/// trigger.trigger();
/// signal.wait().await;
/// # });
/// ```
pub fn channel() -> (ShutdownTrigger, ShutdownSignal) {
    let (sender, receiver) = watch::channel(false);
    (ShutdownTrigger(sender), ShutdownSignal(receiver))
}

impl ShutdownTrigger {
    /// Notifies every [`ShutdownSignal`], including ones cloned later.
    pub fn trigger(&self) {
        self.0.send_replace(true);
    }
}

impl ShutdownSignal {
    /// Resolves once shutdown is triggered. Never resolves if the trigger is dropped
    /// without firing, so dropping the trigger does not tear down live sessions.
    pub async fn wait(&mut self) {
        if self.0.wait_for(|stopping| *stopping).await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn wait_after_trigger_resolves() {
        let (trigger, mut signal) = channel();

        trigger.trigger();

        signal.wait().await;
    }

    #[tokio::test]
    async fn wait_on_clone_made_after_trigger_resolves() {
        let (trigger, signal) = channel();
        trigger.trigger();

        let mut late = signal.clone();

        late.wait().await;
    }

    #[tokio::test(start_paused = true)]
    async fn wait_with_dropped_trigger_never_resolves() {
        let (trigger, mut signal) = channel();

        drop(trigger);

        let waited = tokio::time::timeout(Duration::from_secs(1), signal.wait()).await;
        assert!(waited.is_err());
    }
}
