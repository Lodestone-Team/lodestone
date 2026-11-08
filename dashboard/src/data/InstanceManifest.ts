import { QueryClient, useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { ConfigurableManifest } from 'bindings/ConfigurableManifest';
import { ConfigurableValue } from 'bindings/ConfigurableValue';

export const instanceManifestKey = (uuid: string) => [
  'instances',
  uuid,
  'settings',
];

/**
 * The settings of an instance. Fetched again every time the settings page
 * opens (they also change from the CLI or the game), but the last copy is kept
 * meanwhile, so reopening the page shows it at once instead of a loading state.
 * Changes made here are written into it with setCachedInstanceSetting.
 */
export const useInstanceManifest = (uuid: string) => {
  return useQuery<ConfigurableManifest, AxiosError>({
    queryKey: instanceManifestKey(uuid),
    queryFn: () => {
      return axios
        .get<ConfigurableManifest>(`/instance/${uuid}/settings`)
        .then((response) => response.data);
    },
    staleTime: 0,
  });
};

/** Updates a setting's value in the cached settings, after it was saved. */
export const setCachedInstanceSetting = (
  queryClient: QueryClient,
  uuid: string,
  sectionId: string,
  settingId: string,
  value: ConfigurableValue | null
) => {
  queryClient.setQueryData<ConfigurableManifest>(
    instanceManifestKey(uuid),
    (manifest) => {
      const section = manifest?.setting_sections[sectionId];
      const setting = section?.settings[settingId];
      if (!manifest || !section || !setting) return manifest;
      return {
        ...manifest,
        setting_sections: {
          ...manifest.setting_sections,
          [sectionId]: {
            ...section,
            settings: {
              ...section.settings,
              [settingId]: { ...setting, value },
            },
          },
        },
      };
    }
  );
};
