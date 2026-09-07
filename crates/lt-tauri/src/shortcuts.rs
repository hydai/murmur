use std::future::Future;
use tauri_plugin_global_shortcut::Shortcut;
use tokio::sync::OwnedMutexGuard;

pub(crate) trait Registry {
    fn register(&self, shortcut: Shortcut) -> Result<(), String>;
    fn unregister(&self, shortcut: Shortcut) -> Result<(), String>;
    fn is_registered(&self, shortcut: Shortcut) -> bool;
}

/// Registers the new shortcut, releases the previous one, and only then
/// persists the change. Any failure restores the previous registration and
/// leaves the disk untouched, so settings and active bindings never disagree.
pub(crate) async fn replace(
    registry: &impl Registry,
    old: Option<Shortcut>,
    new: Shortcut,
    persist: impl Future<Output = Result<(), String>>,
) -> Result<(), String> {
    if old == Some(new) && registry.is_registered(new) {
        return persist.await;
    }
    registry.register(new)?;
    // A previous shortcut that never registered (for example, taken by another
    // app at startup) has nothing to release.
    let released = old.filter(|old| *old != new && registry.is_registered(*old));
    if let Some(old) = released {
        if let Err(error) = registry.unregister(old) {
            if let Err(cleanup) = registry.unregister(new) {
                tracing::warn!("Failed to roll back new shortcut: {cleanup}");
            }
            return Err(format!("Failed to release previous shortcut: {error}"));
        }
    }
    if let Err(error) = persist.await {
        let mut restored = true;
        if let Some(old) = released {
            if let Err(restore) = registry.register(old) {
                restored = false;
                tracing::warn!("Failed to restore previous shortcut: {restore}");
            }
        }
        if restored {
            if let Err(cleanup) = registry.unregister(new) {
                tracing::warn!("Failed to roll back new shortcut: {cleanup}");
            }
        } else {
            tracing::warn!(
                "Keeping the new shortcut registered: the previous one could not be restored"
            );
        }
        return Err(error);
    }
    Ok(())
}

/// Runs the replacement on a task the caller does not own. Cancelling the
/// caller can no longer strand both registrations while the disk write
/// completes; the update guard stays held until the transaction settles.
pub(crate) async fn replace_detached<R>(
    registry: R,
    updates: OwnedMutexGuard<()>,
    old: Option<Shortcut>,
    new: Shortcut,
    persist: impl Future<Output = Result<(), String>> + Send + 'static,
) -> Result<(), String>
where
    R: Registry + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let _updates = updates;
        replace(&registry, old, new, persist).await
    })
    .await
    .map_err(|error| format!("Shortcut update task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Default)]
    struct FakeRegistry {
        active: Mutex<Vec<Shortcut>>,
        reject_new: bool,
        reject_release: Option<Shortcut>,
    }

    impl Registry for FakeRegistry {
        fn register(&self, shortcut: Shortcut) -> Result<(), String> {
            if self.reject_new {
                return Err("shortcut unavailable".into());
            }
            self.active.lock().unwrap().push(shortcut);
            Ok(())
        }
        fn unregister(&self, shortcut: Shortcut) -> Result<(), String> {
            if self.reject_release == Some(shortcut) {
                return Err("platform refused to release shortcut".into());
            }
            let mut active = self.active.lock().unwrap();
            if !active.contains(&shortcut) {
                return Err("shortcut is not registered".into());
            }
            active.retain(|item| *item != shortcut);
            Ok(())
        }
        fn is_registered(&self, shortcut: Shortcut) -> bool {
            self.active.lock().unwrap().contains(&shortcut)
        }
    }

    #[tokio::test]
    async fn registration_failure_preserves_old_without_writing() {
        let old = "Ctrl+A".parse().unwrap();
        let new = "Ctrl+B".parse().unwrap();
        let registry = FakeRegistry {
            active: Mutex::new(vec![old]),
            reject_new: true,
            ..Default::default()
        };
        let result = replace(&registry, Some(old), new, async {
            panic!("must not persist");
        })
        .await;
        assert!(result.is_err());
        assert_eq!(*registry.active.lock().unwrap(), vec![old]);
    }

    #[tokio::test]
    async fn persistence_failure_rolls_back_new_registration() {
        let old = "Ctrl+A".parse().unwrap();
        let new = "Ctrl+B".parse().unwrap();
        let registry = FakeRegistry {
            active: Mutex::new(vec![old]),
            reject_new: false,
            ..Default::default()
        };
        assert!(
            replace(&registry, Some(old), new, async { Err("disk full".into()) })
                .await
                .is_err()
        );
        assert_eq!(*registry.active.lock().unwrap(), vec![old]);
    }

    #[tokio::test]
    async fn previous_shortcut_is_released_before_persisting() {
        let old = "Ctrl+A".parse().unwrap();
        let new = "Ctrl+B".parse().unwrap();
        let registry = FakeRegistry {
            active: Mutex::new(vec![old]),
            reject_new: false,
            ..Default::default()
        };
        replace(&registry, Some(old), new, async {
            // Disk changes only once the registrations are in their final state.
            assert!(!registry.is_registered(old));
            assert!(registry.is_registered(new));
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(*registry.active.lock().unwrap(), vec![new]);
    }

    #[tokio::test]
    async fn previous_shortcut_that_was_never_registered_is_skipped() {
        // Startup registration can fail (combination taken by another app);
        // choosing a new shortcut must still work from that state.
        let old = "Ctrl+A".parse().unwrap();
        let new = "Ctrl+B".parse().unwrap();
        let registry = FakeRegistry::default();
        replace(&registry, Some(old), new, async { Ok(()) })
            .await
            .unwrap();
        assert_eq!(*registry.active.lock().unwrap(), vec![new]);
    }

    #[tokio::test]
    async fn failure_to_release_previous_shortcut_restores_state_without_persisting() {
        let old = "Ctrl+A".parse().unwrap();
        let new = "Ctrl+B".parse().unwrap();
        let registry = FakeRegistry {
            active: Mutex::new(vec![old]),
            reject_new: false,
            reject_release: Some(old),
        };
        let result = replace(&registry, Some(old), new, async {
            panic!("must not persist");
        })
        .await;
        assert!(result.is_err());
        assert_eq!(*registry.active.lock().unwrap(), vec![old]);
    }

    impl Registry for Arc<FakeRegistry> {
        fn register(&self, shortcut: Shortcut) -> Result<(), String> {
            (**self).register(shortcut)
        }
        fn unregister(&self, shortcut: Shortcut) -> Result<(), String> {
            (**self).unregister(shortcut)
        }
        fn is_registered(&self, shortcut: Shortcut) -> bool {
            (**self).is_registered(shortcut)
        }
    }

    #[tokio::test]
    async fn cancelled_caller_does_not_interrupt_replacement() {
        let old: Shortcut = "Ctrl+A".parse().unwrap();
        let new: Shortcut = "Ctrl+B".parse().unwrap();
        let registry = Arc::new(FakeRegistry {
            active: Mutex::new(vec![old]),
            reject_new: false,
            ..Default::default()
        });
        let updates = Arc::new(tokio::sync::Mutex::new(()));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel::<()>();
        let guard = updates.clone().lock_owned().await;
        let task_registry = registry.clone();
        let caller = tokio::spawn(async move {
            replace_detached(task_registry, guard, Some(old), new, async move {
                let _ = started_tx.send(());
                release_rx.await.unwrap();
                Ok(())
            })
            .await
        });
        started_rx.await.unwrap();
        caller.abort();
        let _ = caller.await;

        // The transaction outlives its caller: it still holds the update lock,
        // and the registrations already reflect the pending change.
        assert!(updates.try_lock().is_err());
        assert!(!registry.is_registered(old));
        assert!(registry.is_registered(new));

        release_tx.send(()).unwrap();
        let settled = tokio::time::timeout(Duration::from_secs(2), async {
            while updates.try_lock().is_err() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            settled.is_ok(),
            "replacement did not finish after the caller was cancelled"
        );
        assert_eq!(*registry.active.lock().unwrap(), vec![new]);
    }
}
