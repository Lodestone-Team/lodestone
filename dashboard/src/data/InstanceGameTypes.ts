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
