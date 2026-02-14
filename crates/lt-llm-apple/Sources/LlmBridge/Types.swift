import Foundation

/// Callback invoked when LLM processing completes successfully.
/// `ctx` is the opaque pointer passed to `llm_bridge_process`.
/// `text` is a UTF-8 C string containing the response (caller must NOT free it).
public typealias LlmCompletionCallback = @convention(c) (
    _ ctx: UnsafeMutableRawPointer?,
    _ text: UnsafePointer<CChar>?
) -> Void

/// Callback invoked when LLM processing fails.
/// `ctx` is the opaque pointer passed to `llm_bridge_process`.
/// `message` is a UTF-8 C string describing the error (caller must NOT free it).
public typealias LlmErrorCallback = @convention(c) (
    _ ctx: UnsafeMutableRawPointer?,
    _ message: UnsafePointer<CChar>?
) -> Void
