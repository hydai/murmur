use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::process::Command;

/// Permission status enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionStatus {
    Granted,
    Denied,
    NotDetermined,
    Restricted,
    Unknown,
}

/// Result of permission check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsResult {
    pub microphone: PermissionStatus,
    pub accessibility: PermissionStatus,
}

#[cfg(any(target_os = "macos", test))]
fn microphone_status(authorization_status: isize) -> PermissionStatus {
    // AVAuthorizationStatus, declared in AVFoundation/AVCaptureDevice.h.
    match authorization_status {
        0 => PermissionStatus::NotDetermined,
        1 => PermissionStatus::Restricted,
        2 => PermissionStatus::Denied,
        3 => PermissionStatus::Granted,
        _ => PermissionStatus::Unknown,
    }
}

#[cfg(target_os = "macos")]
mod native {
    use block2::RcBlock;
    use objc2::{class, msg_send, runtime::AnyObject, runtime::Bool};
    use std::sync::Mutex;
    use tokio::sync::oneshot;

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        // AVMediaType is an NSString pointer. Use the framework constant instead
        // of guessing the string representation of the media type.
        static AVMediaTypeAudio: *const AnyObject;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        // CoreServices Boolean is an unsigned byte, not Objective-C BOOL.
        fn AXIsProcessTrusted() -> u8;
    }

    pub fn microphone_authorization_status() -> isize {
        // SAFETY: These signatures match AVCaptureDevice.h. AVMediaTypeAudio
        // is a non-null NSString constant owned by the linked framework, and
        // the class method is available on every supported macOS version.
        unsafe {
            msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: AVMediaTypeAudio]
        }
    }

    pub fn accessibility_is_trusted() -> bool {
        // SAFETY: AXIsProcessTrusted takes no arguments, queries this process,
        // and does not display a permission prompt.
        unsafe { AXIsProcessTrusted() != 0 }
    }

    pub fn request_microphone_access() -> oneshot::Receiver<bool> {
        let (sender, receiver) = oneshot::channel();
        let sender = Mutex::new(Some(sender));
        let completion = RcBlock::new(move |granted: Bool| {
            // The callback can run on any dispatch queue. Send at most once,
            // and permit cancellation if the awaiting IPC call was dropped.
            if let Some(sender) = sender
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                let _ = sender.send(granted.as_bool());
            }
        });

        // SAFETY: The class method signature and BOOL callback match the SDK.
        // AVFoundation copies the escaping completion block before returning,
        // retaining its sender until the user responds. The Rust RcBlock is
        // confined to this synchronous function, so it never crosses an await.
        unsafe {
            let _: () = msg_send![class!(AVCaptureDevice),
                requestAccessForMediaType: AVMediaTypeAudio,
                completionHandler: &*completion
            ];
        }
        receiver
    }
}

/// Query the current app's microphone authorization, without spawning a child.
#[cfg(target_os = "macos")]
pub fn check_microphone_permission() -> PermissionStatus {
    microphone_status(native::microphone_authorization_status())
}

/// Query accessibility trust for this process without triggering a prompt.
#[cfg(target_os = "macos")]
pub fn check_accessibility_permission() -> PermissionStatus {
    if native::accessibility_is_trusted() {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    }
}

#[cfg(any(target_os = "macos", test))]
fn microphone_request_result(granted: bool) -> Result<(), String> {
    if granted {
        Ok(())
    } else {
        Err("Microphone access denied. Enable Murmur in System Settings > Privacy & Security > Microphone.".to_string())
    }
}

/// Request microphone permission and wait asynchronously for the user's answer.
#[cfg(target_os = "macos")]
pub async fn request_microphone_permission() -> Result<(), String> {
    match check_microphone_permission() {
        PermissionStatus::Granted => return Ok(()),
        PermissionStatus::Denied => return microphone_request_result(false),
        PermissionStatus::Restricted => {
            return Err("Microphone access is restricted by system policy.".to_string())
        }
        PermissionStatus::Unknown => {
            return Err("Unknown microphone authorization status.".to_string())
        }
        PermissionStatus::NotDetermined => {}
    }

    let granted = native::request_microphone_access()
        .await
        .map_err(|_| "Microphone authorization request ended without a response.".to_string())?;
    microphone_request_result(granted)
}

/// Open System Preferences to a specific pane
#[cfg(target_os = "macos")]
pub fn open_system_preferences(section: &str) -> Result<(), String> {
    let url = match section {
        "microphone" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        "accessibility" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        _ => return Err(format!("Unknown preferences section: {}", section)),
    };

    let status = Command::new("open")
        .arg(url)
        .status()
        .map_err(|e| format!("Failed to open System Preferences: {}", e))?;
    if !status.success() {
        return Err(format!("Failed to open System Preferences: {}", status));
    }

    Ok(())
}

// Non-macOS stubs
#[cfg(not(target_os = "macos"))]
pub fn check_microphone_permission() -> PermissionStatus {
    PermissionStatus::Granted
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permission() -> PermissionStatus {
    PermissionStatus::Granted
}

#[cfg(not(target_os = "macos"))]
pub async fn request_microphone_permission() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn open_system_preferences(_section: &str) -> Result<(), String> {
    Err("System Preferences only available on macOS".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "queries actual OS authorization for the test process"]
    fn test_check_permissions() {
        // Just verify functions don't panic
        let _ = check_microphone_permission();
        let _ = check_accessibility_permission();
    }

    #[tokio::test]
    #[ignore = "may display a real microphone authorization prompt"]
    async fn test_request_permission() {
        let result = request_microphone_permission().await;
        assert!(result.is_ok());
    }

    #[test]
    fn microphone_authorization_status_mapping() {
        assert_eq!(microphone_status(0), PermissionStatus::NotDetermined);
        assert_eq!(microphone_status(1), PermissionStatus::Restricted);
        assert_eq!(microphone_status(2), PermissionStatus::Denied);
        assert_eq!(microphone_status(3), PermissionStatus::Granted);
        assert_eq!(microphone_status(-1), PermissionStatus::Unknown);
        assert_eq!(microphone_status(4), PermissionStatus::Unknown);
    }

    #[test]
    fn microphone_request_reports_denial_instead_of_success() {
        assert!(microphone_request_result(true).is_ok());
        assert!(microphone_request_result(false)
            .unwrap_err()
            .contains("denied"));
    }

    #[test]
    fn permission_status_keeps_existing_ipc_representation() {
        assert_eq!(
            serde_json::to_string(&PermissionStatus::NotDetermined).unwrap(),
            "\"notdetermined\""
        );
        assert_eq!(
            serde_json::to_string(&PermissionStatus::Granted).unwrap(),
            "\"granted\""
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_open_preferences_invalid_section() {
        let result = open_system_preferences("invalid");
        assert!(result.is_err());
    }
}
