use tokio::process::Command;
use tokio::task::JoinHandle;

const START_SOUND: &str = "/System/Library/Sounds/Tink.aiff";
const STOP_SOUND: &str = "/System/Library/Sounds/Pop.aiff";

/// Spawn a fire-and-forget child that still gets collected.
///
/// `std::process::Child` has no `Drop` reaper, so spawning `afplay` and
/// dropping the handle left a zombie behind for every start and stop sound —
/// two per recording, for the lifetime of the app. Awaiting the child on the
/// runtime reaps it as soon as it exits.
///
/// Returns the waiting task so tests can observe completion; callers ignore it.
fn spawn_and_reap(mut command: Command, label: &'static str) -> Option<JoinHandle<()>> {
    let handle = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => {
            tracing::debug!("No runtime available to run {label}");
            return None;
        }
    };
    match command.spawn() {
        Ok(mut child) => Some(handle.spawn(async move {
            if let Err(error) = child.wait().await {
                tracing::debug!("{label} failed: {error}");
            }
        })),
        Err(error) => {
            tracing::debug!("Failed to start {label}: {error}");
            None
        }
    }
}

fn play(path: &'static str) -> Option<JoinHandle<()>> {
    let mut command = Command::new("afplay");
    command.arg(path);
    spawn_and_reap(command, "sound playback")
}

pub fn play_start_sound() {
    play(START_SOUND);
}

pub fn play_stop_sound() {
    play(STOP_SOUND);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A silent stand-in for `afplay`, so the reaping behaviour is covered
    /// without making the test machine play a sound.
    fn silent_child() -> Command {
        Command::new("/usr/bin/true")
    }

    #[tokio::test]
    async fn a_spawned_child_is_waited_on_instead_of_being_left_as_a_zombie() {
        let task = spawn_and_reap(silent_child(), "test child").expect("spawning the child");
        tokio::time::timeout(std::time::Duration::from_secs(10), task)
            .await
            .expect("the child must be reaped")
            .expect("the waiting task must not panic");
    }

    #[tokio::test]
    async fn a_missing_program_is_reported_rather_than_propagated() {
        assert!(spawn_and_reap(Command::new("/nonexistent/afplay"), "missing").is_none());
    }

    #[test]
    fn playback_outside_a_runtime_is_skipped_rather_than_panicking() {
        assert!(play(STOP_SOUND).is_none());
    }
}
