import { useQuery, QueryClient } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';

export const updateInstance = (
  uuid: string,
  queryClient: QueryClient,
  updater: (oldInfo: InstanceInfo) => InstanceInfo
) => {
  queryClient.setQueriesData(
    ['instances', 'list'],
    (oldData: { [uuid: string]: InstanceInfo } | undefined) => {
      if (!oldData) return oldData;
      return {
        ...oldData,
        [uuid]: updater(oldData[uuid]),
      };
    }
  );
};

export const useInstanceList = () =>
  useQuery<{ [uuid: string]: InstanceInfo }, AxiosError>(
    ['instances', 'list'],
    () => {
      return axios
        .get<InstanceInfo[]>('/instance/list')
        .then((response) => {
          return response.data.reduce(
            (acc, instance) => ({
              ...acc,
              [instance.uuid]: instance,
            }),
            {}
          );
        });
    },
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
