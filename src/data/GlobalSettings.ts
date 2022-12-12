import { GlobalSettings } from './../bindings/GlobalSettings';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';

export const useGlobalSettings = () =>
  useQuery<GlobalSettings, AxiosError>(
    ['global_settings'],
    () => {
      return axios.get<GlobalSettings>('/global_settings').then((response) => {
        return response.data;
      });
    },
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
