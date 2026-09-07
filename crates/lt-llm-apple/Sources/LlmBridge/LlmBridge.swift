import Foundation
import FoundationModels

// MARK: - Sendable wrappers for FFI types

/// Wraps a non-Sendable raw pointer for use across concurrency boundaries.
/// Safety: The wrapped pointer is set once at init and only read thereafter.
struct SendablePointer: @unchecked Sendable {
    let value: UnsafeMutableRawPointer?
}

/// Wraps C function pointer callbacks for Sendable conformance.
/// Safety: C function pointers are stateless and inherently thread-safe.
struct SendableCompletionCB: @unchecked Sendable {
    let fn: LlmCompletionCallback
}

struct SendableErrorCB: @unchecked Sendable {
    let fn: LlmErrorCallback
}

/// Rust owns one retained handle until completion or cancellation. The task
/// owns its copied inputs and callback context independently of this handle.
private final class LlmRequest {
    let task: Task<Void, Never>

    init(task: Task<Void, Never>) {
        self.task = task
    }
}

// MARK: - @_cdecl entry points

@_cdecl("llm_bridge_is_available")
public func llmBridgeIsAvailable() -> Bool {
    guard #available(macOS 26, *) else { return false }
    return SystemLanguageModel.default.isAvailable
}

@_cdecl("llm_bridge_process")
public func llmBridgeProcess(
    _ instructions: UnsafePointer<CChar>?,
    _ prompt: UnsafePointer<CChar>?,
    _ ctx: UnsafeMutableRawPointer?,
    _ onComplete: LlmCompletionCallback?,
    _ onError: LlmErrorCallback?
) -> UnsafeMutableRawPointer? {
    guard #available(macOS 26, *) else {
        let msg = "Apple Foundation Models requires macOS 26+"
        msg.withCString { onError?(ctx, $0) }
        return nil
    }

    guard let instructions = instructions,
          let prompt = prompt,
          let onComplete = onComplete,
          let onError = onError else {
        let msg = "Invalid arguments: instructions, prompt, and callbacks are required"
        msg.withCString { onError?(ctx, $0) }
        return nil
    }

    let instructionsStr = String(cString: instructions)
    let promptStr = String(cString: prompt)
    let capturedCtx = SendablePointer(value: ctx)
    let capturedOnComplete = SendableCompletionCB(fn: onComplete)
    let capturedOnError = SendableErrorCB(fn: onError)

    let task = Task {
        do {
            try Task.checkCancellation()
            let session = LanguageModelSession(
                instructions: instructionsStr
            )
            let response = try await session.respond(to: promptStr)
            try Task.checkCancellation()
            let text = String(response.content)
            text.withCString { capturedOnComplete.fn(capturedCtx.value, $0) }
        } catch {
            let msg = "Apple LLM error: \(error.localizedDescription)"
            msg.withCString { capturedOnError.fn(capturedCtx.value, $0) }
        }
    }
    return Unmanaged.passRetained(LlmRequest(task: task)).toOpaque()
}

/// Consumes the retained request handle. Cancellation is cooperative; the
/// task still invokes exactly one callback, which releases Rust's context.
@_cdecl("llm_bridge_cancel")
public func llmBridgeCancel(_ handle: UnsafeMutableRawPointer?) {
    guard let handle = handle else { return }
    let request = Unmanaged<LlmRequest>.fromOpaque(handle).takeRetainedValue()
    request.task.cancel()
}

@_cdecl("llm_bridge_free_string")
public func llmBridgeFreeString(_ ptr: UnsafeMutablePointer<CChar>?) {
    guard let ptr = ptr else { return }
    free(ptr)
}
