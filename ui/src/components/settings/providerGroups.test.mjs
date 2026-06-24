import assert from 'node:assert/strict';
import test from 'node:test';

import { groupSttProviders } from './providerGroups.ts';

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
