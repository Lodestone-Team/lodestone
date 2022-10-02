import { useContext, useState, useEffect } from 'react';
// React Query hooks for systeminfo

import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
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
  return useQuery<MemInfo, AxiosError>(
    ['systeminfo', 'meminfo'],
    () => axios.get<MemInfo>(`/system/memory`).then((res) => res.data),
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
};

export interface DiskInfo {
  total: number;
  free: number;
}

export const useDiskInfo = () => {
  return useQuery<DiskInfo, AxiosError>(
    ['systeminfo', 'diskinfo'],
    () => axios.get<DiskInfo>(`/system/disk`).then((res) => res.data),
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
};

export interface CPUInfo {
  cpu_vendor: string;
  cpu_speed: number;
  cpu_load: number;
}

export const useCPUInfo = () => {
  return useQuery<CPUInfo, AxiosError>(
    ['systeminfo', 'cpuinfo'],
    () => axios.get<CPUInfo>(`/system/cpu`).then((res) => res.data),
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
};

export interface ClientInfo{
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
  client_name: string;
  up_since: number;
}

export const useClientInfo = () => {
  return useQuery<ClientInfo, AxiosError>(
    ['systeminfo', 'clientinfo'],
    () => axios.get<ClientInfo>(`/info`).then((res) => res.data),
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
};
