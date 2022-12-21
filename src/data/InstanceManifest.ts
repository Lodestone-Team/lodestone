import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { InstanceManifest } from 'bindings/InstanceManifest';

export const useInstanceManifest = (uuid: string) => {
  return useQuery<InstanceManifest, AxiosError>(
    ['instances', uuid, 'manifest'],
    () => {
      return axios
        .get<InstanceManifest>(`/instance/${uuid}/manifest`)
        .then((response) => response.data);
    },
    {
      staleTime: 0,
      cacheTime: 0,
    }
  );
};
