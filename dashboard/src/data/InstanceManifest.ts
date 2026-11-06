import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { ConfigurableManifest } from 'bindings/ConfigurableManifest';

export const useInstanceManifest = (uuid: string) => {
  return useQuery<ConfigurableManifest, AxiosError>({
    queryKey: ['instances', uuid, 'settings'],
    queryFn: () => {
      return axios
        .get<ConfigurableManifest>(`/instance/${uuid}/settings`)
        .then((response) => response.data);
    },
    staleTime: 0,
    gcTime: 0,
  });
};
