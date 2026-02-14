#ifndef LLM_BRIDGE_H
#define LLM_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>

/// Callback for successful LLM completion.
/// `ctx` is the opaque pointer passed to `llm_bridge_process`.
/// `text` is a UTF-8 C string (caller must NOT free it).
typedef void (*LlmCompletionCallback)(
    void *ctx,
    const char *text
);

/// Callback for LLM processing errors.
/// `ctx` is the opaque pointer passed to `llm_bridge_process`.
/// `message` is a UTF-8 C string (caller must NOT free it).
typedef void (*LlmErrorCallback)(
    void *ctx,
    const char *message
);

/// Check if Apple Foundation Models (FoundationModels framework) is available.
/// Returns true if the system language model is available on this device.
bool llm_bridge_is_available(void);

/// Process text using Apple Foundation Models.
/// Creates a LanguageModelSession with the given system instructions,
/// sends the prompt, and invokes the appropriate callback with the result.
/// This call blocks until the LLM responds.
///
/// `instructions` - system instructions for the session (UTF-8 C string).
/// `prompt` - the user prompt to process (UTF-8 C string).
/// `ctx` - opaque pointer forwarded to callbacks (caller owns its lifetime).
/// `on_complete` - called with the response text on success.
/// `on_error` - called with an error message on failure.
void llm_bridge_process(
    const char *instructions,
    const char *prompt,
    void *ctx,
    LlmCompletionCallback on_complete,
    LlmErrorCallback on_error
);

/// Free a string previously returned by LlmBridge functions.
void llm_bridge_free_string(char *ptr);

#endif /* LLM_BRIDGE_H */
