import { useQuery, QueryClient } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { t } from 'i18n';

export const updateInstance = (
  uuid: string,
  queryClient: QueryClient,
  updater: (oldInfo: InstanceInfo) => InstanceInfo
) => {
  queryClient.setQueryData(
    ['instances', 'list'],
    (oldData: { [uuid: string]: InstanceInfo } | undefined) => {
      if (!oldData) return oldData;
      if (!oldData[uuid]) return oldData;
      return {
        ...oldData,
        [uuid]: updater(oldData[uuid]),
      };
    }
  );
};

export const addInstance = (
  instanceInfo: InstanceInfo,
  queryClient: QueryClient
) => {
  queryClient.setQueryData(
    ['instances', 'list'],
    (oldData: { [uuid: string]: InstanceInfo } | undefined) => {
      if (!oldData) return oldData;
      return {
        ...oldData,
        [instanceInfo.uuid]: instanceInfo,
      };
    }
  );
};

export const deleteInstance = (uuid: string, queryClient: QueryClient) => {
  queryClient.setQueryData(
    ['instances', 'list'],
    (oldData: { [uuid: string]: InstanceInfo } | undefined) => {
      if (!oldData) return oldData;
      const newData = { ...oldData };
      delete newData[uuid];
      return newData;
    }
  );
};

// instance list sorted by creation time
export const useInstanceList = (enabled = true) =>
  useQuery<{ [uuid: string]: InstanceInfo }, AxiosError>({
    queryKey: ['instances', 'list'],
    queryFn: () => {
      return axios.get<InstanceInfo[]>('/instance/list').then((response) => {
        if (response.status !== 200) {
          throw new Error(t('shell.error.invalidStatusCode'));
        }
        if (!response.data) {
          throw new Error(t('shell.error.invalidResponse'));
        }
        return response.data.reduce(
          (acc, instance) => ({
            ...acc,
            [instance.uuid]: instance,
          }),
          {}
        );
      });
    },
    enabled,
  });
