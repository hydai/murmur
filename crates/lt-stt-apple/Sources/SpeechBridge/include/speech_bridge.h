#ifndef SPEECH_BRIDGE_H
#define SPEECH_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/// Opaque handle to a speech session
typedef struct SpeechSession SpeechSession;

/// Model availability status
typedef enum {
    SpeechModelStatusInstalled = 0,
    SpeechModelStatusNotInstalled = 1,
    SpeechModelStatusDownloading = 2,
    SpeechModelStatusUnavailable = 3,
} SpeechModelStatus;

/// Callback for transcription results.
/// `ctx` is the opaque pointer passed to `speech_bridge_create_session`.
/// `text` is a UTF-8 C string (caller must NOT free it).
/// `timestamp_ms` is the time offset in the audio stream.
/// `is_final` indicates whether this is a committed (final) result.
typedef void (*SpeechTranscriptionCallback)(
    void *ctx,
    const char *text,
    uint64_t timestamp_ms,
    bool is_final
);

/// Callback for errors.
/// `ctx` is the opaque pointer passed to `speech_bridge_create_session`.
/// `message` is a UTF-8 C string (caller must NOT free it).
typedef void (*SpeechErrorCallback)(
    void *ctx,
    const char *message
);

/// Callback for model download progress.
/// `ctx` is the opaque pointer passed to `speech_bridge_download_model`.
/// `progress` is a value between 0.0 and 1.0.
/// `finished` is true when the download is complete.
typedef void (*SpeechModelProgressCallback)(
    void *ctx,
    double progress,
    bool finished
);

/// Check if SpeechTranscriber is available on this system (macOS 26+).
bool speech_bridge_is_available(void);

/// Get supported locales as a JSON array of strings, e.g. ["en_US", "ja_JP"].
/// Caller must free the returned string with `speech_bridge_free_string`.
char *speech_bridge_get_supported_locales(void);

/// Check the model installation status for a given locale.
SpeechModelStatus speech_bridge_check_model_status(const char *locale);

/// Trigger model download for a locale.
/// Progress is reported via the callback; the call returns immediately.
void speech_bridge_download_model(
    const char *locale,
    void *ctx,
    SpeechModelProgressCallback callback
);

/// Create a new speech session for the given locale.
/// Returns NULL on failure.
/// `ctx` is forwarded to both callbacks — the caller owns its lifetime.
SpeechSession *speech_bridge_create_session(
    const char *locale,
    void *ctx,
    SpeechTranscriptionCallback on_transcription,
    SpeechErrorCallback on_error
);

/// Feed PCM audio samples (signed 16-bit, mono, 16 kHz) to the session.
/// `samples` points to `count` int16_t values.
/// `timestamp_ms` is the time offset for this chunk.
/// Returns false if the session is invalid or audio could not be enqueued.
bool speech_bridge_send_audio(
    SpeechSession *session,
    const int16_t *samples,
    size_t count,
    uint64_t timestamp_ms
);

/// Signal end of audio input. The session will finish processing any remaining
/// audio and deliver final transcription results before stopping.
void speech_bridge_stop_session(SpeechSession *session);

/// Destroy the session and free all associated resources.
/// Must be called after `speech_bridge_stop_session`.
void speech_bridge_destroy_session(SpeechSession *session);

/// Free a string previously returned by `speech_bridge_get_supported_locales`.
void speech_bridge_free_string(char *ptr);

#endif /* SPEECH_BRIDGE_H */
