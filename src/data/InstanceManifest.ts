import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { InstanceManifest } from 'bindings/InstanceManifest';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';

export const useInstanceManifest = (uuid: string) => {
  const { isReady } = useContext(LodestoneContext);
  return useQuery<InstanceManifest, AxiosError>(
    ['instances', uuid, 'manifest'],
    () => {
      return axios
        .get<InstanceManifest>(`/instance/${uuid}/manifest`)
        .then((response) => response.data);
    },
    {
      enabled: isReady,
      staleTime: 0,
      cacheTime: 0,
    }
  );
};
