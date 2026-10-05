import assert from 'node:assert/strict';
import test from 'node:test';

import {
  groupSttProviders,
  orderedSttProviders,
  sttProviderAction,
  sttProviderDetail,
  sttProviderDisabled,
} from './providerGroups.ts';

/** A provider as the backend lists it; each test overrides what it is about. */
const provider = (overrides) => ({
  id: 'openai',
  name: 'OpenAI Whisper',
  configured: false,
  provider_type: 'batch',
  requires_api_key: true,
  model_status: null,
  ...overrides,
});

const apple = (model_status) => provider({
  id: 'apple_stt',
  name: 'Apple Speech',
  configured: model_status === 'installed',
  provider_type: 'local',
  requires_api_key: false,
  model_status,
});
const elevenlabs = (configured = false) => provider({
  id: 'elevenlabs', name: 'ElevenLabs Scribe', configured, provider_type: 'streaming',
});
const openai = (configured = false) => provider({ configured });
const groq = (configured = false) => provider({ id: 'groq', name: 'Groq Whisper Turbo', configured });
const custom = (configured = false) => provider({
  id: 'custom_stt', name: 'Custom Endpoint', configured, requires_api_key: false,
});

test('keeps configured custom STT provider in the custom group', () => {
  const providers = [
    {
      id: 'elevenlabs',
      name: 'ElevenLabs Scribe',
      configured: true,
      provider_type: 'streaming',
      requires_api_key: true,
      model_status: null,
    },
    {
      id: 'custom_stt',
      name: 'Local Whisper',
      configured: true,
      provider_type: 'batch',
      requires_api_key: false,
      model_status: null,
    },
  ];

  const grouped = groupSttProviders(providers);

  assert.equal(grouped.cloudProviders.length, 1);
  assert.equal(grouped.cloudProviders[0].id, 'elevenlabs');
  assert.equal(grouped.customProvider?.id, 'custom_stt');
  assert.equal(grouped.customProvider?.name, 'Local Whisper');
});

test('orders the services local, then cloud, then custom', () => {
  const ordered = orderedSttProviders([elevenlabs(), openai(), groq(), apple('installed'), custom()]);
  assert.deepEqual(
    ordered.map((p) => p.id),
    ['apple_stt', 'elevenlabs', 'openai', 'groq', 'custom_stt'],
  );
});

test('keeps the order the backend listed the cloud services in, and drops none', () => {
  const input = [custom(), groq(), openai(), elevenlabs()];
  const ordered = orderedSttProviders(input);
  assert.deepEqual(ordered.map((p) => p.id), ['groq', 'openai', 'elevenlabs', 'custom_stt']);
  assert.equal(ordered.length, input.length);
  // The input is not reordered in place.
  assert.deepEqual(input.map((p) => p.id), ['custom_stt', 'groq', 'openai', 'elevenlabs']);
});

test('orders a list without a custom endpoint, and an empty one', () => {
  assert.deepEqual(orderedSttProviders([openai(), apple('installed')]).map((p) => p.id), ['apple_stt', 'openai']);
  assert.deepEqual(orderedSttProviders([]), []);
});

test('treats a provider without a type as cloud when ordering', () => {
  const bare = { id: 'openai', name: 'OpenAI Whisper', description: 'Cloud transcription', configured: true };
  assert.deepEqual(orderedSttProviders([bare, apple('installed')]).map((p) => p.id), ['apple_stt', 'openai']);
});

test('describes Apple Speech by the state of its model', () => {
  assert.equal(sttProviderDetail(apple('installed'), ''), 'On this Mac · no API key needed');
  assert.equal(sttProviderDetail(apple('not_installed'), ''), 'Speech model not downloaded');
  assert.equal(sttProviderDetail(apple('unavailable'), ''), 'Requires macOS 26');
});

test('describes a model that is being downloaded, or whose state is unknown, without claiming it is installed', () => {
  assert.equal(sttProviderDetail(apple('downloading'), ''), 'Downloading speech model…');
  assert.equal(sttProviderDetail(apple(null), ''), 'Speech model not downloaded');
});

test('describes a cloud service by whether it is live, and whether its key is saved', () => {
  assert.equal(sttProviderDetail(elevenlabs(false), ''), 'Cloud · live');
  assert.equal(sttProviderDetail(openai(false), ''), 'Cloud');
  assert.equal(sttProviderDetail(groq(false), ''), 'Cloud');
  // Both facts are kept once there is a key.
  assert.equal(sttProviderDetail(elevenlabs(true), ''), 'Cloud · live · key saved');
  assert.equal(sttProviderDetail(openai(true), ''), 'Cloud · key saved');
  assert.equal(sttProviderDetail(groq(true), ''), 'Cloud · key saved');
});

test('describes a custom endpoint by its address, or by what it could be', () => {
  // The name is already the one the user gave it, so the address is what is left to say.
  assert.equal(sttProviderDetail(custom(true), 'http://localhost:8080/v1'), 'http://localhost:8080/v1');
  assert.equal(sttProviderDetail(custom(false), ''), 'Any Whisper-compatible server');
  // An address typed into a form but never saved is not the endpoint's.
  assert.equal(sttProviderDetail(custom(false), 'http://typing.test'), 'Any Whisper-compatible server');
});

test('describes a provider the backend sent without its type fields as cloud', () => {
  const bare = { id: 'openai', name: 'OpenAI Whisper', description: 'Cloud transcription', configured: true };
  assert.equal(sttProviderDetail(bare, ''), 'Cloud · key saved');
  assert.equal(sttProviderDetail({ ...bare, configured: false }, ''), 'Cloud');
});

test('offers to download a model that is not installed', () => {
  assert.equal(sttProviderAction(apple('not_installed')), 'download');
});

test('offers to add a key to a provider that needs one and has none', () => {
  assert.equal(sttProviderAction(elevenlabs(false)), 'add-key');
  assert.equal(sttProviderAction(openai(false)), 'add-key');
  assert.equal(sttProviderAction(groq(false)), 'add-key');
});

test('offers to set up a custom endpoint that is not set up', () => {
  assert.equal(sttProviderAction(custom(false)), 'set-up');
});

test('offers nothing when the provider is ready, or cannot be helped from here', () => {
  assert.equal(sttProviderAction(apple('installed')), null);
  assert.equal(sttProviderAction(apple('downloading')), null);
  assert.equal(sttProviderAction(apple('unavailable')), null);
  assert.equal(sttProviderAction(elevenlabs(true)), null);
  assert.equal(sttProviderAction(openai(true)), null);
  assert.equal(sttProviderAction(custom(true)), null);
});

test('offers nothing for a provider the backend sent without its type fields', () => {
  const bare = { id: 'openai', name: 'OpenAI Whisper', description: 'Cloud transcription', configured: true };
  assert.equal(sttProviderAction(bare), null);
  assert.equal(sttProviderAction({ ...bare, configured: false }), null);
});

test('sets up a custom endpoint, not a key, even if it were to ask for one', () => {
  // The endpoint form is where its key goes, so this is never "add-key".
  assert.equal(sttProviderAction({ ...custom(false), requires_api_key: true }), 'set-up');
});

test('disables only a provider this Mac cannot run', () => {
  assert.equal(sttProviderDisabled(apple('unavailable')), true);
  assert.equal(sttProviderDisabled(apple('installed')), false);
  assert.equal(sttProviderDisabled(apple('not_installed')), false);
  assert.equal(sttProviderDisabled(apple('downloading')), false);
  assert.equal(sttProviderDisabled(openai(false)), false);
  assert.equal(sttProviderDisabled(custom(false)), false);
  assert.equal(sttProviderDisabled({ id: 'openai', name: 'OpenAI Whisper', configured: true }), false);
});
