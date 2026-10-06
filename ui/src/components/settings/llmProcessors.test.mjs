import assert from 'node:assert/strict';
import test from 'node:test';

import {
  installHint,
  llmProcessorAction,
  llmProcessorDetail,
  llmProcessorDisabled,
  llmProcessorHint,
  orderedLlmProcessors,
} from './llmProcessors.ts';

/** A processor as the backend lists it; each test overrides what it is about. */
const processor = (overrides) => ({
  name: 'Claude API',
  id: 'claude_api',
  available: false,
  default_model: 'claude-sonnet-4-20250514',
  provider_type: 'http',
  requires_api_key: true,
  configured: false,
  api_key_name: 'anthropic',
  ...overrides,
});

const apple = (available = true) => processor({
  name: 'Apple Intelligence', id: 'apple_llm', available, default_model: '(system default)',
  provider_type: 'local', requires_api_key: false, configured: true, api_key_name: null,
});
const gemini = (available = true) => processor({
  name: 'Gemini CLI', id: 'gemini', available, default_model: 'gemini-3-flash-preview',
  provider_type: 'cli', requires_api_key: false, configured: true, api_key_name: null,
});
const copilot = (available = true) => processor({
  name: 'Copilot CLI', id: 'copilot', available, default_model: 'gpt-5-mini',
  provider_type: 'cli', requires_api_key: false, configured: true, api_key_name: null,
});
const openai = (configured = false) => processor({
  name: 'OpenAI API', id: 'openai_api', configured, available: configured,
  default_model: 'gpt-4o-mini', api_key_name: 'openai',
});
const claude = (configured = false) => processor({ configured, available: configured });
const geminiApi = (configured = false) => processor({
  name: 'Gemini API', id: 'gemini_api', configured, available: configured,
  default_model: 'gemini-2.0-flash', api_key_name: 'google_ai',
});
const custom = (configured = false) => processor({
  name: 'Custom Endpoint', id: 'custom_api', configured, available: configured,
  default_model: 'gpt-4o-mini', provider_type: 'custom', requires_api_key: false, api_key_name: 'custom_llm',
});

test('orders the processors on this Mac, then command-line tools, then cloud, then custom', () => {
  // The order the backend lists them in.
  const listed = [gemini(), copilot(), apple(), openai(), claude(), geminiApi(), custom()];
  assert.deepEqual(
    orderedLlmProcessors(listed).map((p) => p.id),
    ['apple_llm', 'gemini', 'copilot', 'openai_api', 'claude_api', 'gemini_api', 'custom_api'],
  );
});

test('lists a custom endpoint, configured or not', () => {
  // The old page filtered by three kinds and never showed this one.
  assert.ok(orderedLlmProcessors([gemini(), custom(false)]).some((p) => p.id === 'custom_api'));
  assert.ok(orderedLlmProcessors([custom(true), gemini()]).some((p) => p.id === 'custom_api'));
  assert.equal(orderedLlmProcessors([custom(true), gemini(), apple(), claude()]).at(-1).id, 'custom_api');
});

test('keeps the order the backend listed each kind in, drops none, and leaves the input alone', () => {
  const input = [geminiApi(), custom(), copilot(), openai(), gemini(), claude()];
  const ordered = orderedLlmProcessors(input);
  assert.deepEqual(
    ordered.map((p) => p.id),
    ['copilot', 'gemini', 'gemini_api', 'openai_api', 'claude_api', 'custom_api'],
  );
  assert.equal(ordered.length, input.length);
  assert.deepEqual(
    input.map((p) => p.id),
    ['gemini_api', 'custom_api', 'copilot', 'openai_api', 'gemini', 'claude_api'],
  );
});

test('orders an empty list, and a list without every kind', () => {
  assert.deepEqual(orderedLlmProcessors([]), []);
  assert.deepEqual(orderedLlmProcessors([claude(), gemini()]).map((p) => p.id), ['gemini', 'claude_api']);
});

test('treats a processor of a kind it does not know as a cloud service', () => {
  const future = processor({ id: 'future_api', name: 'Future API', provider_type: 'quantum' });
  assert.deepEqual(
    orderedLlmProcessors([custom(), future, gemini(), apple()]).map((p) => p.id),
    ['apple_llm', 'gemini', 'future_api', 'custom_api'],
  );
  assert.equal(llmProcessorDetail(future, ''), 'Cloud');
  assert.equal(llmProcessorAction(future), 'add-key');
  assert.equal(llmProcessorDisabled(future), false);
});

test('describes a processor on this Mac by whether it is available', () => {
  assert.equal(llmProcessorDetail(apple(true), ''), 'On this Mac');
  assert.equal(llmProcessorDetail(apple(false), ''), 'Not available on this Mac');
});

test('describes a command-line tool by whether it is installed, and leaves how to install it to its hint', () => {
  assert.equal(llmProcessorDetail(gemini(true), ''), 'Command-line tool');
  assert.equal(llmProcessorDetail(copilot(true), ''), 'Command-line tool');
  // The line is a button's text, which is cut off when the row is narrow and cannot be copied.
  assert.equal(llmProcessorDetail(gemini(false), ''), 'Not installed');
  assert.equal(llmProcessorDetail(copilot(false), ''), 'Not installed');
  const other = processor({ id: 'other_cli', name: 'Other CLI', provider_type: 'cli', available: false, requires_api_key: false });
  assert.equal(llmProcessorDetail(other, ''), 'Not installed');
});

test('hints how to install a command-line tool that is not installed', () => {
  assert.equal(llmProcessorHint(gemini(false)), 'Install: npm install -g @google/gemini-cli');
  assert.equal(llmProcessorHint(copilot(false)), 'Install: npm install -g @github/copilot');
});

test('has no hint for a tool that is installed, a tool it has none for, or anything that is not a tool', () => {
  assert.equal(llmProcessorHint(gemini(true)), '');
  assert.equal(llmProcessorHint(copilot(true)), '');
  const other = processor({ id: 'other_cli', name: 'Other CLI', provider_type: 'cli', available: false, requires_api_key: false });
  assert.equal(llmProcessorHint(other), '');
  assert.equal(llmProcessorHint(apple(false)), '');
  assert.equal(llmProcessorHint(claude(false)), '');
  assert.equal(llmProcessorHint(openai(false)), '');
  assert.equal(llmProcessorHint(custom(false)), '');
  // Whatever id a processor of another kind has, an install hint is not for it.
  assert.equal(llmProcessorHint({ ...claude(false), id: 'gemini' }), '');
});

test('describes a cloud service by whether its key is saved', () => {
  assert.equal(llmProcessorDetail(claude(false), ''), 'Cloud');
  assert.equal(llmProcessorDetail(openai(false), ''), 'Cloud');
  assert.equal(llmProcessorDetail(geminiApi(false), ''), 'Cloud');
  assert.equal(llmProcessorDetail(claude(true), ''), 'Cloud · key saved');
  assert.equal(llmProcessorDetail(openai(true), ''), 'Cloud · key saved');
  assert.equal(llmProcessorDetail(geminiApi(true), ''), 'Cloud · key saved');
});

test('describes a custom endpoint by its address, or by what it could be', () => {
  // The name is already the one the user gave it, so the address is what is left to say.
  assert.equal(llmProcessorDetail(custom(true), 'http://localhost:11434/v1'), 'http://localhost:11434/v1');
  assert.equal(llmProcessorDetail(custom(false), ''), 'Any OpenAI-compatible server');
  // An address typed into a form but never saved is not the endpoint's.
  assert.equal(llmProcessorDetail(custom(false), 'http://typing.test'), 'Any OpenAI-compatible server');
});

test('offers to add a key to a cloud service that needs one and has none', () => {
  assert.equal(llmProcessorAction(claude(false)), 'add-key');
  assert.equal(llmProcessorAction(openai(false)), 'add-key');
  assert.equal(llmProcessorAction(geminiApi(false)), 'add-key');
});

test('offers to set up a custom endpoint that is not set up', () => {
  assert.equal(llmProcessorAction(custom(false)), 'set-up');
});

test('sets up a custom endpoint, not a key, even if it were to ask for one', () => {
  // The endpoint form is where its key goes, so this is never "add-key".
  assert.equal(llmProcessorAction({ ...custom(false), requires_api_key: true }), 'set-up');
});

test('offers nothing when the processor is ready, or cannot be helped from here', () => {
  assert.equal(llmProcessorAction(apple(true)), null);
  assert.equal(llmProcessorAction(apple(false)), null);
  assert.equal(llmProcessorAction(gemini(true)), null);
  assert.equal(llmProcessorAction(copilot(false)), null);
  assert.equal(llmProcessorAction(claude(true)), null);
  assert.equal(llmProcessorAction(openai(true)), null);
  assert.equal(llmProcessorAction(custom(true)), null);
});

test('disables only a tool or an on-device model that is not available', () => {
  assert.equal(llmProcessorDisabled(apple(false)), true);
  assert.equal(llmProcessorDisabled(gemini(false)), true);
  assert.equal(llmProcessorDisabled(copilot(false)), true);
  assert.equal(llmProcessorDisabled(apple(true)), false);
  assert.equal(llmProcessorDisabled(gemini(true)), false);
});

test('never disables a cloud service or a custom endpoint, whatever the backend says is available', () => {
  // A service with no key is listed as unavailable, and its row is what opens the key sheet.
  assert.equal(llmProcessorDisabled(claude(false)), false);
  assert.equal(llmProcessorDisabled(openai(false)), false);
  assert.equal(llmProcessorDisabled(custom(false)), false);
  assert.equal(llmProcessorDisabled(claude(true)), false);
  assert.equal(llmProcessorDisabled(custom(true)), false);
});

test('gives the install hint of each tool, and none for anything else', () => {
  assert.equal(installHint('gemini'), 'Install: npm install -g @google/gemini-cli');
  assert.equal(installHint('copilot'), 'Install: npm install -g @github/copilot');
  assert.equal(installHint('apple_llm'), '');
  assert.equal(installHint('claude_api'), '');
  assert.equal(installHint(''), '');
});
