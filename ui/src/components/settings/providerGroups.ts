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
