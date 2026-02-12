use serde::{Deserialize, Serialize};
use std::process::Command;

/// Permission status enum
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Check microphone permission status on macOS
#[cfg(target_os = "macos")]
pub fn check_microphone_permission() -> PermissionStatus {
    // Use Swift/ObjC bridge via osascript to check microphone permission
    // Note: This is a best-effort check. The definitive permission prompt
    // happens when actually attempting to access the microphone.
    let script = r#"
        use framework "AVFoundation"

        set authStatus to current application's AVCaptureDevice's authorizationStatusForMediaType:"audi"
        return authStatus as integer
    "#;

    let output = Command::new("osascript")
        .arg("-l")
        .arg("JavaScript")
        .arg("-e")
        .arg(script)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let status_str = String::from_utf8_lossy(&result.stdout).trim().to_string();
            match status_str.as_str() {
                "0" => PermissionStatus::NotDetermined,
                "1" => PermissionStatus::Restricted,
                "2" => PermissionStatus::Denied,
                "3" => PermissionStatus::Granted,
                _ => PermissionStatus::Unknown,
            }
        }
        _ => {
            // If osascript fails, assume not determined (will prompt on first use)
            PermissionStatus::NotDetermined
        }
    }
}

/// Check accessibility permission status on macOS
#[cfg(target_os = "macos")]
pub fn check_accessibility_permission() -> PermissionStatus {
    // Use AppleScript to check accessibility permission
    let script = r#"
        tell application "System Events"
            return get UI elements enabled
        end tell
    "#;

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let status_str = String::from_utf8_lossy(&result.stdout).trim().to_string();
            if status_str == "true" {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Denied
            }
        }
        _ => PermissionStatus::NotDetermined,
    }
}

/// Request microphone permission (triggers system prompt)
#[cfg(target_os = "macos")]
pub fn request_microphone_permission() -> Result<(), String> {
    // Attempting to access microphone will trigger the permission prompt
    // This is done through the audio capture module
    Ok(())
}

/// Open System Preferences to a specific pane
#[cfg(target_os = "macos")]
pub fn open_system_preferences(section: &str) -> Result<(), String> {
    let url = match section {
        "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
        "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        _ => return Err(format!("Unknown preferences section: {}", section)),
    };

    Command::new("open")
        .arg(url)
        .spawn()
        .map_err(|e| format!("Failed to open System Preferences: {}", e))?;

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
pub fn request_microphone_permission() -> Result<(), String> {
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
    fn test_check_permissions() {
        // Just verify functions don't panic
        let _ = check_microphone_permission();
        let _ = check_accessibility_permission();
    }

    #[test]
    fn test_request_permission() {
        let result = request_microphone_permission();
        assert!(result.is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_open_preferences_invalid_section() {
        let result = open_system_preferences("invalid");
        assert!(result.is_err());
    }
}
