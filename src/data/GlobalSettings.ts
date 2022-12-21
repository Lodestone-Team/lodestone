import { GlobalSettings } from './../bindings/GlobalSettings';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';

export const useGlobalSettings = () =>
  useQuery<GlobalSettings, AxiosError>(
    ['global_settings'],
    () => {
      return axios.get<GlobalSettings>('/global_settings').then((response) => {
        return response.data;
      });
    }
  );
