import { HandlerGameType } from 'bindings/HandlerGameType';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { SetupManifest } from 'bindings/SetupManifest';
import { GenericHandlerGameType } from 'components/Instance/InstanceCreateForm';
export const InstanceGameTypes = () =>
  useQuery<HandlerGameType[], AxiosError>({
    queryKey: ['games'],
    queryFn: async () => {
      const response = await axios.get<HandlerGameType[]>('/games');
      return response.data;
    },
  });

export const SetupInstanceManifest = (game_type: HandlerGameType) => {
  return useQuery<SetupManifest, AxiosError>({
    queryKey: ['setup_manifest', game_type],
    queryFn: async () => {
      const response = await axios.get<SetupManifest>(
        `/setup_manifest/${game_type}`
      );
      return response.data;
    },
  });
};

export const SetupGenericInstanceManifest = (
  game_type: GenericHandlerGameType,
  generic_instance_url: string,
  url_is_ready: boolean
) => {
  return useQuery<SetupManifest, AxiosError>({
    queryKey: ['generic_setup_manifest', game_type],
    queryFn: async () => {
      const response = await axios.put<SetupManifest>(
        `/generic_setup_manifest`,
        { url: generic_instance_url }
      );
      return response.data;
    },
    enabled: url_is_ready,
    gcTime: 0,
    retry: false,
  });
};
