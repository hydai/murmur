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

// MARK: - Sync-async bridge helper

/// Runs an async closure synchronously using a semaphore.
/// Used by @_cdecl functions which cannot be async.
private func runBlocking<T: Sendable>(_ body: @Sendable @escaping () async -> T) -> T {
    let semaphore = DispatchSemaphore(value: 0)
    nonisolated(unsafe) var result: T!
    Task {
        result = await body()
        semaphore.signal()
    }
    semaphore.wait()
    return result
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
) {
    guard #available(macOS 26, *) else {
        let msg = "Apple Foundation Models requires macOS 26+"
        msg.withCString { onError?(ctx, $0) }
        return
    }

    guard let instructions = instructions,
          let prompt = prompt,
          let onComplete = onComplete,
          let onError = onError else {
        let msg = "Invalid arguments: instructions, prompt, and callbacks are required"
        msg.withCString { onError?(ctx, $0) }
        return
    }

    let instructionsStr = String(cString: instructions)
    let promptStr = String(cString: prompt)
    let capturedCtx = SendablePointer(value: ctx)
    let capturedOnComplete = SendableCompletionCB(fn: onComplete)
    let capturedOnError = SendableErrorCB(fn: onError)

    runBlocking {
        do {
            let session = LanguageModelSession(
                instructions: instructionsStr
            )
            let response = try await session.respond(to: promptStr)
            let text = String(response.content)
            text.withCString { capturedOnComplete.fn(capturedCtx.value, $0) }
        } catch {
            let msg = "Apple LLM error: \(error.localizedDescription)"
            msg.withCString { capturedOnError.fn(capturedCtx.value, $0) }
        }
    }
}

@_cdecl("llm_bridge_free_string")
public func llmBridgeFreeString(_ ptr: UnsafeMutablePointer<CChar>?) {
    guard let ptr = ptr else { return }
    free(ptr)
}
