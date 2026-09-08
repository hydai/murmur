/**
 * The Tauri event contract, in one place.
 *
 * Each handler used to declare its own `event.payload as { ... }` shape, so a
 * renamed field on the Rust side surfaced as `undefined` at runtime rather
 * than a type error — and two listeners survived for events the backend never
 * emitted. Keeping the names and shapes together makes both visible.
 *
 * Payloads mirror the structs in `crates/lt-tauri/src/events.rs`.
 */

export interface PipelineStatePayload {
  state: string;
  timestamp_ms: number;
}

export interface AudioLevelPayload {
  rms: number;
  voice_active: boolean;
  timestamp_ms: number;
}

export interface TranscriptionPayload {
  text: string;
  timestamp_ms: number;
}

export interface FinalResultPayload {
  text: string;
  processing_time_ms: number;
}

export interface ErrorPayload {
  message: string;
  recoverable: boolean;
}

export interface RecordingStatePayload {
  is_recording: boolean;
}

export interface AudioErrorPayload {
  message: string;
}

export interface CommandDetectedPayload {
  command_name: string | null;
  timestamp_ms: number;
}

export interface ModelProgressPayload {
  locale: string;
  progress: number;
  finished: boolean;
  error: string | null;
}

/** Every event the Rust side emits, and what it carries. */
export interface AppEvents {
  'pipeline-state': PipelineStatePayload;
  'pipeline-result': FinalResultPayload;
  'pipeline-error': ErrorPayload;
  'recording-state': RecordingStatePayload;
  'audio-level': AudioLevelPayload;
  'audio-error': AudioErrorPayload;
  'transcription-partial': TranscriptionPayload;
  'transcription-committed': TranscriptionPayload;
  'command-detected': CommandDetectedPayload;
  'apple-stt-model-progress': ModelProgressPayload;
  'open-settings': void;
  'open-about-and-check': void;
  'update-available': void;
}

export type AppEventName = keyof AppEvents;
