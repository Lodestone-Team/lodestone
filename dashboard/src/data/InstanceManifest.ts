import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { ConfigurableManifest } from 'bindings/ConfigurableManifest';

export const useInstanceManifest = (uuid: string) => {
  return useQuery<ConfigurableManifest, AxiosError>(
    ['instances', uuid, 'settings'],
    () => {
      return axios
        .get<ConfigurableManifest>(`/instance/${uuid}/settings`)
        .then((response) => response.data);
    },
    {
      staleTime: 0,
      cacheTime: 0,
    }
  );
};
