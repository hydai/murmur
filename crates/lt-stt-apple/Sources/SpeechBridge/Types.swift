import Foundation

/// Mirror of the C enum SpeechModelStatus.
/// Raw values MUST match the C header exactly.
public enum ModelStatus: Int32, Sendable {
    case installed = 0
    case notInstalled = 1
    case downloading = 2
    case unavailable = 3
}

/// Type aliases for the C callback signatures — keeps call-sites readable.
public typealias TranscriptionCallback = @convention(c) (
    _ ctx: UnsafeMutableRawPointer?,
    _ text: UnsafePointer<CChar>?,
    _ timestampMs: UInt64,
    _ isFinal: Bool
) -> Void

public typealias ErrorCallback = @convention(c) (
    _ ctx: UnsafeMutableRawPointer?,
    _ message: UnsafePointer<CChar>?
) -> Void

public typealias ModelProgressCallback = @convention(c) (
    _ ctx: UnsafeMutableRawPointer?,
    _ progress: Double,
    _ finished: Bool
) -> Void
