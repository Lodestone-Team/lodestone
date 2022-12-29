import { LODESTONE_PORT } from 'utils/util';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';

export interface MemInfo {
  total: number;
  free: number;
  avail: number;
  buffers: number;
  cached: number;
  swap_total: number;
  swap_free: number;
}

export const useMemInfo = () => {
  return useQuery<MemInfo, AxiosError>(['systeminfo', 'meminfo'], () =>
    axios.get<MemInfo>(`/system/memory`).then((res) => res.data)
  );
};

export interface DiskInfo {
  total: number;
  free: number;
}

export const useDiskInfo = () => {
  return useQuery<DiskInfo, AxiosError>(['systeminfo', 'diskinfo'], () =>
    axios.get<DiskInfo>(`/system/disk`).then((res) => res.data)
  );
};

export interface CPUInfo {
  cpu_vendor: string;
  cpu_speed: number;
  cpu_load: number;
}

export const useCPUInfo = () => {
  return useQuery<CPUInfo, AxiosError>(['systeminfo', 'cpuinfo'], () =>
    axios.get<CPUInfo>(`/system/cpu`).then((res) => res.data)
  );
};

export interface CoreInfo {
  version: string;
  is_setup: boolean;
  os: string;
  arch: string;
  cpu: string;
  cpu_count: number;
  total_ram: number;
  total_disk: number;
  host_name: string;
  uuid: string;
  core_name: string;
  up_since: number;
}

/**
 * Uses react query to fetch the core info
 * Will change the core status to success if the connection is successful
 * Will change the core status to error if the connection is unsuccessful
 */
export const useCoreInfo = () => {
  const { setCoreConnectionStatus } = useContext(LodestoneContext);
  return useQuery<CoreInfo, AxiosError>(
    ['systeminfo', 'CoreInfo'],
    () => axios.get<CoreInfo>(`/info`).then((res) => res.data),
    {
      onSuccess: () => {
        setCoreConnectionStatus('success');
      },
      onError: () => {
        setCoreConnectionStatus('error');
      },
    }
  );
};

// this should only be used to check if the core is setup or not
// it refetches frequently to check if any new core shows up
export const useLocalCoreInfo = () => {
  //change to https when we default to it in core
  return useQuery<CoreInfo, AxiosError>(
    ['systeminfo', 'LocalCoreInfo'],
    () =>
      axios
        .get<CoreInfo>(`/info`, {
          baseURL: `http://localhost:${LODESTONE_PORT}/api/v1`,
          timeout: 3000,
        })
        .then((res) => res.data),
    {
      refetchInterval: 3000,
    }
  );
};
