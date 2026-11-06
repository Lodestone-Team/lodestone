import { HandlerGameType } from 'bindings/HandlerGameType';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { SetupManifest } from 'bindings/SetupManifest';
import type { GenericHandlerGameType } from 'components/Instance/InstanceCreateForm';
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

export const useGenericSetupInstanceManifest = (
  game_type: GenericHandlerGameType,
  generic_instance_url: string,
  enabled: boolean
) => {
  return useQuery<SetupManifest, AxiosError>({
    // the manifest depends on the URL, a different URL is a different manifest
    queryKey: ['generic_setup_manifest', game_type, generic_instance_url],
    queryFn: async () => {
      const response = await axios.put<SetupManifest>(
        `/generic_setup_manifest`,
        { url: generic_instance_url }
      );
      return response.data;
    },
    enabled,
    gcTime: 0,
    retry: false,
  });
};
