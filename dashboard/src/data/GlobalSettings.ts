import { GlobalSettingsData } from './../@bindings/GlobalSettingsData';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';

export const useGlobalSettings = () =>
  useQuery<GlobalSettingsData, AxiosError>(
    ['global_settings'],
    () => {
      return axios.get<GlobalSettingsData>('/global_settings').then((response) => {
        return response.data;
      });
    }
  );
