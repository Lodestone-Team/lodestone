import axios, { AxiosError } from 'axios';
import { useQuery } from '@tanstack/react-query';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';

export const useGameSetting = (uuid: string, setting: string) =>
  useQuery<string, AxiosError>(
    ['instances', uuid, 'settings', 'game', setting],
    () => {
      return axios
        .get<string>(`/instance/${uuid}/game/${setting}`)
        .then((response) => {
          return response.data;
        });
    },
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
