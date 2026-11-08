import { coreApiBaseUrl, DEFAULT_LOCAL_CORE } from 'utils/util';
import { tauri } from 'utils/tauriUtil';
import { useQueryFetchCallbacks } from 'utils/hooks';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { TlsInfo } from 'bindings/TlsInfo';
import { CoreCapabilities } from 'bindings/CoreCapabilities';
import { t } from 'i18n';

// The /system routes require login; axios sends the token by default
// (set in pages/index.tsx).

/** Bytes. `free` is the memory available to start new programs. */
export interface MemInfo {
  total: number;
  free: number;
}

export const useMemInfo = () => {
  return useQuery<MemInfo, AxiosError>({
    queryKey: ['systeminfo', 'meminfo'],
    queryFn: () => axios.get<MemInfo>(`/system/ram`).then((res) => res.data),
  });
};

/** Bytes, summed over all disks. */
export interface DiskInfo {
  total: number;
  free: number;
}

export const useDiskInfo = () => {
  return useQuery<DiskInfo, AxiosError>({
    queryKey: ['systeminfo', 'diskinfo'],
    queryFn: () => axios.get<DiskInfo>(`/system/disk`).then((res) => res.data),
  });
};

/** `cpu_speed` is the mean frequency in MHz, `cpu_load` the mean usage in percent. */
export interface CPUInfo {
  cpu_speed: number;
  cpu_load: number;
}

export const useCPUInfo = () => {
  return useQuery<CPUInfo, AxiosError>({
    queryKey: ['systeminfo', 'cpuinfo'],
    queryFn: () => axios.get<CPUInfo>(`/system/cpu`).then((res) => res.data),
  });
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
  /** The core's HTTPS certificate. Missing on cores without HTTPS support. */
  tls?: TlsInfo;
  /** The core's optional features. Missing on older cores, which have them all. */
  capabilities?: Partial<CoreCapabilities>;
}

/**
 * Uses react query to fetch the core info
 * Will change the core status to success if the connection is successful
 * Will change the core status to error if the connection is unsuccessful
 */
export const useCoreInfo = (refetchInterval: number | false = false) => {
  const { setCoreConnectionStatus, coreConnectionStatus } =
    useContext(LodestoneContext);
  useQueryFetchCallbacks<CoreInfo, AxiosError>(['systeminfo', 'CoreInfo'], {
    onSuccess: () => {
      if (coreConnectionStatus !== 'degraded')
        setCoreConnectionStatus('success');
    },
    onError: () => {
      setCoreConnectionStatus('error');
    },
  });
  return useQuery<CoreInfo, AxiosError>({
    queryKey: ['systeminfo', 'CoreInfo'],
    queryFn: () => axios.get<CoreInfo>(`/info`).then((res) => res.data),
    refetchInterval,
  });
};

/** Shown instead of the macro features on a core built without Deno. */
export const noScriptingMessage = () => t('shell.noScripting');

/**
 * The optional features of the core, from /info. A feature the core doesn't
 * report (an older core) counts as available. While /info loads, `isPending`
 * is true: wait before showing anything that may turn out to be unavailable.
 *
 * - `macros`: macros, tasks, instance scripts and extensions.
 * - `genericInstances`: atom instances. Docker instances don't need it.
 */
export const useCoreCapabilities = () => {
  const { data, isPending } = useCoreInfo();
  const capabilities = data?.capabilities;
  return {
    isPending,
    macros: capabilities?.macros ?? true,
    genericInstances: capabilities?.generic_instances ?? true,
  };
};

/**
 * Whether this dashboard can reach a core on this machine: in the desktop
 * app, or when the dashboard page itself is served over http. An https page
 * can't call the local core (plain http), browsers block it as mixed content.
 */
export const canReachLocalCore = () =>
  !!tauri ||
  (typeof window !== 'undefined' && window.location.protocol === 'http:');

// this should only be used to check if the core is setup or not
// it polls until a local core answers, to detect a newly installed core
export const useLocalCoreInfo = (enabled = true) => {
  // plain http: cores always accept it from this machine
  return useQuery<CoreInfo, AxiosError>({
    queryKey: ['systeminfo', 'LocalCoreInfo'],
    queryFn: () =>
      axios
        .get<CoreInfo>(`/info`, {
          baseURL: coreApiBaseUrl(DEFAULT_LOCAL_CORE),
          timeout: 2500,
        })
        .then((res) => res.data),
    enabled,
    retry: false,
    // stop once the local core is known
    refetchInterval: (query) => (query.state.data ? false : 3000),
  });
};
