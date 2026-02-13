import AVFoundation
import Foundation
import Speech

// MARK: - Sendable wrappers for FFI types

/// Wraps a non-Sendable raw pointer for use across concurrency boundaries.
/// Safety: The wrapped pointer is set once at init and only read thereafter.
struct SendablePointer: @unchecked Sendable {
    let value: UnsafeMutableRawPointer?
}

/// Wraps C function pointer callbacks for Sendable conformance.
/// Safety: C function pointers are stateless and inherently thread-safe.
struct SendableTranscriptionCB: @unchecked Sendable {
    let fn: TranscriptionCallback
}

struct SendableErrorCB: @unchecked Sendable {
    let fn: ErrorCallback
}

// MARK: - SpeechSession

/// Wraps a single `SpeechTranscriber` session with C-compatible callbacks.
final class SpeechSession: @unchecked Sendable {
    // Callbacks + context — set once at init, never mutated.
    private let ctx: SendablePointer
    private let onTranscription: SendableTranscriptionCB
    private let onError: SendableErrorCB
    private let locale: Locale

    // The continuation used to push audio into the AsyncStream.
    private let audioContinuation: AsyncStream<AnalyzerInput>.Continuation

    // Background task for iterating transcription results.
    private let resultTask: Task<Void, Never>

    // Cumulative audio timeline in nanoseconds.
    // Each buffer's start time = previous buffer's end time.
    // This prevents timestamp overlap errors from SpeechAnalyzer.
    private var audioTimelineNs: Int64 = 0

    init?(
        localeIdentifier: String,
        ctx: UnsafeMutableRawPointer?,
        onTranscription: @escaping TranscriptionCallback,
        onError: @escaping ErrorCallback
    ) {
        self.ctx = SendablePointer(value: ctx)
        self.onTranscription = SendableTranscriptionCB(fn: onTranscription)
        self.onError = SendableErrorCB(fn: onError)
        self.locale = Locale(identifier: localeIdentifier)

        // Build the audio async stream that feeds into SpeechAnalyzer.
        var cont: AsyncStream<AnalyzerInput>.Continuation!
        let stream = AsyncStream<AnalyzerInput> { cont = $0 }
        self.audioContinuation = cont

        // Create the transcriber with progressive preset for streaming results.
        guard SpeechTranscriber.isAvailable else {
            return nil
        }

        let transcriber = SpeechTranscriber(
            locale: self.locale,
            preset: .progressiveTranscription
        )

        // Capture Sendable values for the task closure.
        let capturedCtx = self.ctx
        let capturedOnTranscription = self.onTranscription
        let capturedOnError = self.onError

        // Spawn a Task that:
        //   1. Creates a SpeechAnalyzer with the audio stream
        //   2. Iterates transcription results
        self.resultTask = Task {
            do {
                let analyzer = SpeechAnalyzer(
                    inputSequence: stream,
                    modules: [transcriber]
                )

                // Iterate transcription results from the transcriber module.
                for try await result in transcriber.results {
                    let text = String(result.text.characters)
                    guard !text.isEmpty else { continue }

                    let isFinal = result.isFinal
                    // Convert CMTimeRange start to milliseconds.
                    let startSeconds = CMTimeGetSeconds(result.range.start)
                    let timestampMs = UInt64(max(0, startSeconds) * 1000)

                    text.withCString { cstr in
                        capturedOnTranscription.fn(capturedCtx.value, cstr, timestampMs, isFinal)
                    }
                }
                // Keep analyzer alive until results are exhausted.
                _ = analyzer
            } catch {
                if !Task.isCancelled {
                    let msg = "SpeechTranscriber error: \(error.localizedDescription)"
                    msg.withCString { capturedOnError.fn(capturedCtx.value, $0) }
                }
            }
        }
    }

    /// Convert PCM i16 16kHz mono samples into an `AVAudioPCMBuffer`,
    /// then push it into the async stream as an `AnalyzerInput`.
    func sendAudio(samples: UnsafePointer<Int16>, count: Int, timestampMs: UInt64) -> Bool {
        // Create an AVAudioFormat for 16-bit int, 16kHz mono.
        guard let inputFormat = AVAudioFormat(
            commonFormat: .pcmFormatInt16,
            sampleRate: 16000,
            channels: 1,
            interleaved: true
        ) else { return false }

        // Create the input buffer from the raw samples.
        guard let inputBuffer = AVAudioPCMBuffer(
            pcmFormat: inputFormat,
            frameCapacity: AVAudioFrameCount(count)
        ) else { return false }

        inputBuffer.frameLength = AVAudioFrameCount(count)

        // Copy the i16 samples into the buffer.
        guard let destPtr = inputBuffer.int16ChannelData?[0] else { return false }
        destPtr.update(from: samples, count: count)

        // Compute monotonic non-overlapping start time from cumulative sample count.
        // Each buffer starts exactly where the previous one ended — no overlap, no gaps.
        // The `timestampMs` parameter from Rust is ignored (wall clock ≠ audio timeline).
        let startTime = CMTime(value: CMTimeValue(audioTimelineNs), timescale: 1_000_000_000)
        let durationNs = Int64(count) * 1_000_000_000 / 16000  // samples / 16kHz in nanoseconds
        audioTimelineNs += durationNs

        let input = AnalyzerInput(buffer: inputBuffer, bufferStartTime: startTime)
        audioContinuation.yield(input)
        return true
    }

    /// Signal end of audio input.
    func stop() {
        audioContinuation.finish()
    }

    /// Cancel the result iteration task and clean up.
    func destroy() {
        audioContinuation.finish()
        resultTask.cancel()
    }
}

// MARK: - Sync-async bridge helpers

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

@_cdecl("speech_bridge_is_available")
public func speechBridgeIsAvailable() -> Bool {
    guard #available(macOS 26, *) else { return false }
    return SpeechTranscriber.isAvailable
}

@_cdecl("speech_bridge_get_supported_locales")
public func speechBridgeGetSupportedLocales() -> UnsafeMutablePointer<CChar>? {
    guard #available(macOS 26, *) else {
        return strdup("[]")
    }

    let localeIds: [String] = runBlocking {
        let locales = await SpeechTranscriber.supportedLocales
        return locales.map { $0.identifier }
    }

    guard let data = try? JSONEncoder().encode(localeIds),
          let json = String(data: data, encoding: .utf8) else {
        return strdup("[]")
    }
    return strdup(json)
}

@_cdecl("speech_bridge_check_model_status")
public func speechBridgeCheckModelStatus(_ locale: UnsafePointer<CChar>?) -> Int32 {
    guard #available(macOS 26, *) else {
        return ModelStatus.unavailable.rawValue
    }

    guard let locale = locale else { return ModelStatus.unavailable.rawValue }
    let localeStr = String(cString: locale)

    let status: ModelStatus = runBlocking {
        let loc = Locale(identifier: localeStr)
        let transcriber = SpeechTranscriber(
            locale: loc,
            preset: .progressiveTranscription
        )
        let assetStatus = await AssetInventory.status(forModules: [transcriber])
        switch assetStatus {
        case .installed:
            return .installed
        case .downloading:
            return .downloading
        case .supported:
            return .notInstalled
        case .unsupported:
            return .unavailable
        @unknown default:
            return .notInstalled
        }
    }

    return status.rawValue
}

@_cdecl("speech_bridge_download_model")
public func speechBridgeDownloadModel(
    _ locale: UnsafePointer<CChar>?,
    _ ctx: UnsafeMutableRawPointer?,
    _ callback: ModelProgressCallback?
) {
    guard #available(macOS 26, *) else {
        callback?(ctx, 0.0, true)
        return
    }

    guard let locale = locale else {
        callback?(ctx, 0.0, true)
        return
    }

    let localeStr = String(cString: locale)
    let capturedCtx = SendablePointer(value: ctx)
    let capturedCallback = callback

    Task {
        do {
            let loc = Locale(identifier: localeStr)
            let transcriber = SpeechTranscriber(
                locale: loc,
                preset: .progressiveTranscription
            )
            guard let request = try await AssetInventory.assetInstallationRequest(
                supporting: [transcriber]
            ) else {
                // Already installed or unsupported.
                capturedCallback?(capturedCtx.value, 1.0, true)
                return
            }

            // Start the download.
            let progress = request.progress
            Task {
                do {
                    try await request.downloadAndInstall()
                    capturedCallback?(capturedCtx.value, 1.0, true)
                } catch {
                    capturedCallback?(capturedCtx.value, 0.0, true)
                }
            }

            // Poll progress until complete.
            while !progress.isFinished && !progress.isCancelled {
                capturedCallback?(capturedCtx.value, progress.fractionCompleted, false)
                try await Task.sleep(for: .milliseconds(500))
            }
        } catch {
            capturedCallback?(capturedCtx.value, 0.0, true)
        }
    }
}

@_cdecl("speech_bridge_create_session")
public func speechBridgeCreateSession(
    _ locale: UnsafePointer<CChar>?,
    _ ctx: UnsafeMutableRawPointer?,
    _ onTranscription: TranscriptionCallback?,
    _ onError: ErrorCallback?
) -> UnsafeMutableRawPointer? {
    guard #available(macOS 26, *) else { return nil }
    guard let locale = locale,
          let onTranscription = onTranscription,
          let onError = onError else { return nil }

    let localeStr = String(cString: locale)
    guard let session = SpeechSession(
        localeIdentifier: localeStr,
        ctx: ctx,
        onTranscription: onTranscription,
        onError: onError
    ) else {
        return nil
    }

    // Move session to the heap so it survives across FFI calls.
    let ptr = Unmanaged.passRetained(session).toOpaque()
    return UnsafeMutableRawPointer(ptr)
}

@_cdecl("speech_bridge_send_audio")
public func speechBridgeSendAudio(
    _ session: UnsafeMutableRawPointer?,
    _ samples: UnsafePointer<Int16>?,
    _ count: Int,
    _ timestampMs: UInt64
) -> Bool {
    guard let session = session, let samples = samples, count > 0 else { return false }
    let obj = Unmanaged<SpeechSession>.fromOpaque(session).takeUnretainedValue()
    return obj.sendAudio(samples: samples, count: count, timestampMs: timestampMs)
}

@_cdecl("speech_bridge_stop_session")
public func speechBridgeStopSession(_ session: UnsafeMutableRawPointer?) {
    guard let session = session else { return }
    let obj = Unmanaged<SpeechSession>.fromOpaque(session).takeUnretainedValue()
    obj.stop()
}

@_cdecl("speech_bridge_destroy_session")
public func speechBridgeDestroySession(_ session: UnsafeMutableRawPointer?) {
    guard let session = session else { return }
    let obj = Unmanaged<SpeechSession>.fromOpaque(session)
    obj.takeRetainedValue().destroy()
}

@_cdecl("speech_bridge_free_string")
public func speechBridgeFreeString(_ ptr: UnsafeMutablePointer<CChar>?) {
    guard let ptr = ptr else { return }
    free(ptr)
}
