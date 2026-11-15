import { HandlerGameType } from 'bindings/HandlerGameType';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { SetupManifest } from 'bindings/SetupManifest';
export const InstanceGameTypes = () =>
  useQuery<HandlerGameType[], AxiosError>({
    queryKey: ['games'],
    queryFn: async () => {
      const response = await axios.get<HandlerGameType[]>('/games');
      return response.data;
    },
  });

export const useSetupInstanceManifest = (
  game_type: HandlerGameType,
  enabled = true
) => {
  return useQuery<SetupManifest, AxiosError>({
    queryKey: ['setup_manifest', game_type],
    queryFn: async () => {
      const response = await axios.get<SetupManifest>(
        `/setup_manifest/${game_type}`
      );
      return response.data;
    },
    enabled,
  });
};

/**
 * `PUT /generic_setup_manifest`: adds a Docker container to the core's watch
 * list (`url` names the container) and returns an empty setup manifest.
 * Docker instances are not atoms, so this stays now that atoms are gone.
 * Nothing in the dashboard calls it at the moment.
 */
export const useGenericSetupInstanceManifest = (
  url: string,
  enabled: boolean
) => {
  return useQuery<SetupManifest, AxiosError>({
    // the manifest depends on the URL, a different URL is a different manifest
    queryKey: ['generic_setup_manifest', url],
    queryFn: async () => {
      const response = await axios.put<SetupManifest>(
        `/generic_setup_manifest`,
        { url }
      );
      return response.data;
    },
    enabled,
    gcTime: 0,
    retry: false,
  });
};
