// Erasable-syntax TypeScript with no imports, because llmProcessors.test.mjs
// loads this file with Node's type stripping.

export interface LlmProcessorInfo {
  name: string;
  id: string;
  available: boolean;
  default_model: string;
  provider_type: 'cli' | 'local' | 'http' | 'custom';
  requires_api_key: boolean;
  configured: boolean;
  api_key_name: string | null;
}

/** What the button at the end of a processor's row does; `null` when the row has none. */
export type LlmProcessorAction = 'add-key' | 'set-up' | null;

type Kind = LlmProcessorInfo['provider_type'];

/** The order the list shows the kinds in. */
const KIND_ORDER: readonly Kind[] = ['local', 'cli', 'http', 'custom'];

/**
 * The kind a processor is shown as. A kind this build does not know, which a
 * newer backend could send, is a cloud service: it keeps its row, and nothing
 * the backend lists can vanish from the page.
 */
function kindOf(processor: LlmProcessorInfo): Kind {
  return KIND_ORDER.includes(processor.provider_type) ? processor.provider_type : 'http';
}

/**
 * The processors in one list: on this Mac, then command-line tools, then cloud
 * services, then the user's own endpoint. Each kind keeps the order the backend
 * listed it in, which the stable sort preserves.
 */
export function orderedLlmProcessors(list: LlmProcessorInfo[]): LlmProcessorInfo[] {
  return [...list].sort((a, b) => KIND_ORDER.indexOf(kindOf(a)) - KIND_ORDER.indexOf(kindOf(b)));
}

/**
 * The second line of a processor's row. `customBaseUrl` is the saved address of
 * the custom endpoint; it is only shown once that endpoint is configured.
 */
export function llmProcessorDetail(processor: LlmProcessorInfo, customBaseUrl: string): string {
  switch (kindOf(processor)) {
    case 'local':
      return processor.available ? 'On this Mac' : 'Not available on this Mac';
    case 'cli':
      // How to install it is `llmProcessorHint`: a line here is cut off in a narrow window and cannot be selected.
      return processor.available ? 'Command-line tool' : 'Not installed';
    case 'custom':
      // The row's name is already the display name the user chose.
      return processor.configured ? customBaseUrl : 'Any OpenAI-compatible server';
    default:
      return processor.configured ? 'Cloud · key saved' : 'Cloud';
  }
}

/**
 * What a row says under itself rather than on its second line: how to get a
 * command-line tool that is not installed. It is text of its own, so it wraps
 * and can be copied; empty when there is nothing to say.
 */
export function llmProcessorHint(processor: LlmProcessorInfo): string {
  return kindOf(processor) === 'cli' && !processor.available ? installHint(processor.id) : '';
}

/** What the row's button is for. A custom endpoint is asked about first: its form is where any key goes. */
export function llmProcessorAction(processor: LlmProcessorInfo): LlmProcessorAction {
  const kind = kindOf(processor);
  if (kind === 'custom') return processor.configured ? null : 'set-up';
  if (kind === 'http' && processor.requires_api_key && !processor.configured) return 'add-key';
  return null;
}

/**
 * A tool or on-device model this Mac cannot run: its row is shown, but cannot
 * be chosen. A cloud service without a key reports itself unavailable too, and
 * its row is what opens the key sheet, so only these two kinds are disabled.
 */
export function llmProcessorDisabled(processor: LlmProcessorInfo): boolean {
  const kind = kindOf(processor);
  return (kind === 'cli' || kind === 'local') && !processor.available;
}

/** How to get a command-line tool that is not installed; empty for anything else. */
export function installHint(id: string): string {
  if (id === 'gemini') return 'Install: npm install -g @google/gemini-cli';
  if (id === 'copilot') return 'Install: npm install -g @github/copilot';
  return '';
}
