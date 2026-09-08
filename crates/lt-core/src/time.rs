//! Wall-clock helpers shared by the crates that stamp events.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch.
///
/// A clock set before 1970 yields 0 rather than panicking: timestamps here only
/// order and label events, so a bad clock must not take down audio capture or
/// the pipeline mid-recording.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ms_is_a_plausible_epoch_millisecond_value() {
        let now = now_ms();
        // 2020-01-01 and 2200-01-01, i.e. wide enough never to be flaky but
        // narrow enough to catch a unit mix-up (seconds or nanoseconds).
        assert!(now > 1_577_836_800_000, "{now}");
        assert!(now < 7_258_118_400_000, "{now}");
    }

    #[test]
    fn now_ms_does_not_go_backwards() {
        let first = now_ms();
        assert!(now_ms() >= first);
    }
}
