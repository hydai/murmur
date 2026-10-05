export interface Provider {
  id: string;
  name: string;
  configured: boolean;
  provider_type: string;
  requires_api_key: boolean;
  model_status: string | null;
}

export interface ProviderGroups {
  localProviders: Provider[];
  cloudProviders: Provider[];
  customProvider: Provider | undefined;
}

export function groupSttProviders(providers: Provider[]): ProviderGroups {
  return {
    localProviders: providers.filter((provider) => provider.provider_type === 'local'),
    cloudProviders: providers.filter(
      (provider) => provider.provider_type !== 'local' && provider.id !== 'custom_stt'
    ),
    customProvider: providers.find((provider) => provider.id === 'custom_stt'),
  };
}

/** What the button at the end of a provider's row does; `null` when the row has none. */
export type ProviderAction = 'download' | 'add-key' | 'set-up' | null;

/**
 * The providers in one list: on this Mac, then in the cloud, then the user's
 * own endpoint. Each kind keeps the order the backend listed it in.
 *
 * The helpers below read `provider_type`, `requires_api_key` and `model_status`
 * only by comparing them, so a provider sent without them is a cloud provider
 * that needs nothing, rather than a crash.
 */
export function orderedSttProviders(providers: Provider[]): Provider[] {
  const { localProviders, cloudProviders, customProvider } = groupSttProviders(providers);
  return [...localProviders, ...cloudProviders, ...(customProvider ? [customProvider] : [])];
}

/**
 * The second line of a provider's row. `customBaseUrl` is the saved address of
 * the custom endpoint; it is only shown once that endpoint is configured.
 */
export function sttProviderDetail(provider: Provider, customBaseUrl: string): string {
  if (provider.id === 'apple_stt') {
    switch (provider.model_status) {
      case 'installed':
        return 'On this Mac · no API key needed';
      case 'unavailable':
        return 'Requires macOS 26';
      case 'downloading':
        return 'Downloading speech model…';
      default:
        return 'Speech model not downloaded';
    }
  }
  if (provider.id === 'custom_stt') {
    // The row's name is already the display name the user chose.
    return provider.configured ? customBaseUrl : 'Any Whisper-compatible server';
  }
  const kind = provider.provider_type === 'streaming' ? 'Cloud · live' : 'Cloud';
  return provider.configured ? `${kind} · key saved` : kind;
}

/** What the row's button is for. A custom endpoint is asked about first: its form is where any key goes. */
export function sttProviderAction(provider: Provider): ProviderAction {
  if (provider.id === 'custom_stt' && !provider.configured) return 'set-up';
  if (provider.model_status === 'not_installed') return 'download';
  if (provider.requires_api_key && !provider.configured) return 'add-key';
  return null;
}

/** A provider this Mac cannot run: its row is shown, but cannot be chosen. */
export function sttProviderDisabled(provider: Provider): boolean {
  return provider.model_status === 'unavailable';
}
