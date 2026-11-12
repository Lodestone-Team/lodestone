import { AvailableUpdate } from 'bindings/AvailableUpdate';
import { ModpackHistory } from 'bindings/ModpackHistory';
import { ModpackUpdatePlan } from 'bindings/ModpackUpdatePlan';
import { ModpackUpdateRequest } from 'bindings/ModpackUpdateRequest';
import { ModpackUpdateResult } from 'bindings/ModpackUpdateResult';
import { UpdateSource } from 'bindings/UpdateSource';
import { ClientPackDownload } from 'bindings/ClientPackDownload';
import { keepPreviousData, QueryClient, useQuery } from '@tanstack/react-query';
import { CreateModpackInstanceRequest } from 'bindings/CreateModpackInstanceRequest';
import { InstalledMod } from 'bindings/InstalledMod';
import { InstalledModList } from 'bindings/InstalledModList';
import { InstanceUuid } from 'bindings/InstanceUuid';
import { ModInstallRequest } from 'bindings/ModInstallRequest';
import { ModInstallResult } from 'bindings/ModInstallResult';
import { ModpackInfo } from 'bindings/ModpackInfo';
import { ModpackInspectRequest } from 'bindings/ModpackInspectRequest';
import { ModrinthProject } from 'bindings/ModrinthProject';
import { ModrinthProjectType } from 'bindings/ModrinthProjectType';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import { ModrinthSearchResponse } from 'bindings/ModrinthSearchResponse';
import { ModrinthUrlTarget } from 'bindings/ModrinthUrlTarget';
import { ModrinthVersion } from 'bindings/ModrinthVersion';
import { SetClientMark } from 'bindings/SetClientMark';
import { fileDownloadUrl } from 'utils/apis';
import { ApiError, axiosWrapper } from 'utils/util';

/*
 * Modrinth browsing (proxied by the core), the mods and plugins of an
 * instance, and modpacks (MODS_SPEC sections 1 to 3). Requests throw
 * ApiError, like the helpers in utils/apis.ts.
 */

// The core waits up to 20 s for Modrinth, longer than the 5 s default.
const MODRINTH_TIMEOUT = 30_000;
// Installs, uploads and modpack downloads take as long as the download.
const NO_TIMEOUT = 0;

/* Modrinth */

export type ModrinthSearchParams = {
  query?: string;
  project_type: ModrinthProjectType;
  game_version?: string;
  loader?: string;
  index?: ModrinthSearchIndex;
  offset?: number;
  limit?: number;
};

export type ModrinthVersionFilter = {
  game_version?: string;
  loader?: string;
};

export const modrinthKeys = {
  search: (params: ModrinthSearchParams) => ['modrinth', 'search', params],
  project: (idOrSlug: string) => ['modrinth', 'project', idOrSlug],
  versions: (idOrSlug: string, filter: ModrinthVersionFilter) => [
    'modrinth',
    'versions',
    idOrSlug,
    filter,
  ],
  version: (versionId: string) => ['modrinth', 'version', versionId],
  resolve: (url: string) => ['modrinth', 'resolve', url],
};

// The core caches Modrinth responses for 5 minutes; so does the dashboard.
const MODRINTH_STALE_TIME = 5 * 60 * 1000;

export const useModrinthSearch = (
  params: ModrinthSearchParams,
  enabled = true
) =>
  useQuery<ModrinthSearchResponse, ApiError>({
    queryKey: modrinthKeys.search(params),
    queryFn: ({ signal }) =>
      axiosWrapper<ModrinthSearchResponse>({
        method: 'get',
        url: '/modrinth/search',
        params,
        timeout: MODRINTH_TIMEOUT,
        signal,
      }),
    enabled,
    staleTime: MODRINTH_STALE_TIME,
    // keep showing the last page while the next one loads
    placeholderData: keepPreviousData,
    retry: false,
  });

export const useModrinthProject = (idOrSlug: string | null) =>
  useQuery<ModrinthProject, ApiError>({
    queryKey: modrinthKeys.project(idOrSlug ?? ''),
    queryFn: ({ signal }) =>
      axiosWrapper<ModrinthProject>({
        method: 'get',
        url: `/modrinth/project/${encodeURIComponent(idOrSlug ?? '')}`,
        timeout: MODRINTH_TIMEOUT,
        signal,
      }),
    enabled: !!idOrSlug,
    staleTime: MODRINTH_STALE_TIME,
    retry: false,
  });

/** Newest first. Without a filter, every version of the project. */
export const useModrinthVersions = (
  idOrSlug: string | null,
  filter: ModrinthVersionFilter
) =>
  useQuery<ModrinthVersion[], ApiError>({
    queryKey: modrinthKeys.versions(idOrSlug ?? '', filter),
    queryFn: ({ signal }) =>
      axiosWrapper<ModrinthVersion[]>({
        method: 'get',
        url: `/modrinth/project/${encodeURIComponent(idOrSlug ?? '')}/versions`,
        params: filter,
        timeout: MODRINTH_TIMEOUT,
        signal,
      }),
    enabled: !!idOrSlug,
    staleTime: MODRINTH_STALE_TIME,
    retry: false,
    // changing the filter ("Show all versions") keeps the list on screen
    // until the new one arrives; never another project's versions
    placeholderData: (previous, previousQuery) =>
      previousQuery?.queryKey[2] === idOrSlug ? previous : undefined,
  });

export const useModrinthVersion = (versionId: string | null) =>
  useQuery<ModrinthVersion, ApiError>({
    queryKey: modrinthKeys.version(versionId ?? ''),
    queryFn: ({ signal }) =>
      axiosWrapper<ModrinthVersion>({
        method: 'get',
        url: `/modrinth/version/${encodeURIComponent(versionId ?? '')}`,
        timeout: MODRINTH_TIMEOUT,
        signal,
      }),
    enabled: !!versionId,
    staleTime: MODRINTH_STALE_TIME,
    retry: false,
  });

/** Resolves a modrinth.com link to its project (and version, if any). */
export const useModrinthResolve = (url: string | null) =>
  useQuery<ModrinthUrlTarget, ApiError>({
    queryKey: modrinthKeys.resolve(url ?? ''),
    queryFn: ({ signal }) =>
      axiosWrapper<ModrinthUrlTarget>({
        method: 'get',
        url: '/modrinth/resolve',
        params: { url },
        timeout: MODRINTH_TIMEOUT,
        signal,
      }),
    enabled: !!url,
    staleTime: MODRINTH_STALE_TIME,
    retry: false,
  });

/* Installed mods */

export const installedModsKey = (uuid: string) => ['instance', uuid, 'mods'];

/**
 * The mods or plugins of an instance. With checkUpdates, the core also asks
 * Modrinth for updates (slower), and each match's `update` is filled in.
 */
export const useInstalledMods = (
  uuid: string,
  checkUpdates: boolean,
  enabled: boolean
) =>
  useQuery<InstalledModList, ApiError>({
    queryKey: [...installedModsKey(uuid), { checkUpdates }],
    queryFn: ({ signal }) =>
      axiosWrapper<InstalledModList>({
        method: 'get',
        url: `/instance/${uuid}/mods`,
        params: checkUpdates ? { check_updates: true } : undefined,
        timeout: MODRINTH_TIMEOUT,
        signal,
      }),
    enabled,
    staleTime: 0,
    retry: false,
    placeholderData: keepPreviousData,
  });

/** Refetches the mod list of an instance after a change. */
export const invalidateInstalledMods = (
  queryClient: QueryClient,
  uuid: string
) => queryClient.invalidateQueries({ queryKey: installedModsKey(uuid) });

const modFileUrl = (uuid: string, fileName: string) =>
  `/instance/${uuid}/mods/${encodeURIComponent(fileName)}`;

/** @throws ApiError */
export const installMod = (uuid: string, request: ModInstallRequest) =>
  axiosWrapper<ModInstallResult>({
    method: 'post',
    url: `/instance/${uuid}/mods/install`,
    data: request,
    timeout: NO_TIMEOUT,
  });

/**
 * Uploads jars into the instance's mods or plugins folder. Throws an
 * ApiError with status 409 if a file exists and overwrite is false.
 * @throws ApiError
 */
export const uploadMods = (uuid: string, files: File[], overwrite: boolean) => {
  const formData = new FormData();
  files.forEach((file) => formData.append('file', file));
  return axiosWrapper<InstalledMod[]>({
    method: 'post',
    url: `/instance/${uuid}/mods/upload`,
    params: overwrite ? { overwrite: true } : undefined,
    data: formData,
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: NO_TIMEOUT,
  });
};

/** @throws ApiError */
export const setModEnabled = (
  uuid: string,
  fileName: string,
  enabled: boolean
) =>
  axiosWrapper<InstalledMod>({
    method: 'put',
    url: `${modFileUrl(uuid, fileName)}/${enabled ? 'enable' : 'disable'}`,
  });

/**
 * Marks a mod as needed by players, or takes the mark off. A marked mod, and
 * the mods it depends on, go into the client modpack (Fabric only; the core
 * refuses plugins).
 * @throws ApiError
 */
export const setClientMark = (
  uuid: string,
  fileName: string,
  required: boolean
) =>
  axiosWrapper<null>({
    method: 'put',
    url: `${modFileUrl(uuid, fileName)}/client`,
    data: { required } satisfies SetClientMark,
  });

/** @throws ApiError */
export const deleteMod = (uuid: string, fileName: string) =>
  axiosWrapper<null>({
    method: 'delete',
    url: modFileUrl(uuid, fileName),
  });

/**
 * The request that builds an instance's client modpack and returns a
 * download key for it, like `/instance/{uuid}/fs/{path}/url` does for files.
 * The route is not final (CLIENT_MODS_SPEC 1.3); it is only named here.
 */
const clientPackKeyRequest = (uuid: string) => ({
  method: 'get',
  url: `/instance/${uuid}/mods/client_pack`,
});

/**
 * Downloads the client modpack (.mrpack) of a Fabric instance, through the
 * same download links the Files tab uses.
 * @throws ApiError
 */
export const downloadClientPack = async (uuid: string) => {
  // building the pack hashes every jar, which can take a while
  const { key } = await axiosWrapper<ClientPackDownload>({
    ...clientPackKeyRequest(uuid),
    timeout: NO_TIMEOUT,
  });
  window.open(fileDownloadUrl(key), '_blank');
};

/* Modpacks */

/** Downloads a Modrinth modpack version on the core and reads it. */
export const inspectModpack = (versionId: string) =>
  axiosWrapper<ModpackInfo>({
    method: 'post',
    url: '/modpack/inspect',
    data: { version_id: versionId } satisfies ModpackInspectRequest,
    timeout: NO_TIMEOUT,
  });

/** Uploads a .mrpack to the core and reads it. */
export const uploadModpack = (file: File) => {
  const formData = new FormData();
  formData.append('file', file);
  return axiosWrapper<ModpackInfo>({
    method: 'post',
    url: '/modpack/upload',
    data: formData,
    headers: { 'Content-Type': 'multipart/form-data' },
    timeout: NO_TIMEOUT,
  });
};

/**
 * Starts creating an instance from a modpack. The core creates it in the
 * background and reports progress through progression events.
 */
export const createModpackInstance = (request: CreateModpackInstanceRequest) =>
  axiosWrapper<InstanceUuid>({
    method: 'post',
    url: '/instance/create_modpack',
    data: request,
    timeout: NO_TIMEOUT,
  });

/* A server's modpack: updates, restores and its history */

export const modpackHistoryKey = (uuid: string) => [
  'instance',
  uuid,
  'modpack',
  'history',
];

/**
 * Every install, update and restore of the server's modpack, newest first,
 * the version each current file came with, and what the core's last update
 * check found.
 */
export const useModpackHistory = (uuid: string, enabled: boolean) =>
  useQuery<ModpackHistory, ApiError>({
    queryKey: modpackHistoryKey(uuid),
    queryFn: ({ signal }) =>
      axiosWrapper<ModpackHistory>({
        method: 'get',
        url: `/instance/${uuid}/modpack/history`,
        signal,
      }),
    enabled,
    retry: false,
  });

/** Looks for a newer version of the server's modpack on Modrinth now. */
export const checkModpack = (uuid: string) =>
  axiosWrapper<AvailableUpdate | null>({
    method: 'post',
    url: `/instance/${uuid}/modpack/check`,
    timeout: MODRINTH_TIMEOUT,
  });

/** What updating (or restoring) the server to `source` would change. */
export const planModpackUpdate = (uuid: string, source: UpdateSource) =>
  axiosWrapper<ModpackUpdatePlan>({
    method: 'post',
    url: `/instance/${uuid}/modpack/update/plan`,
    data: { source } satisfies ModpackUpdateRequest,
    timeout: NO_TIMEOUT,
  });

/** Updates (or restores) the stopped server to `source`; resolves when done. */
export const updateModpack = (uuid: string, source: UpdateSource) =>
  axiosWrapper<ModpackUpdateResult>({
    method: 'post',
    url: `/instance/${uuid}/modpack/update`,
    data: { source } satisfies ModpackUpdateRequest,
    timeout: NO_TIMEOUT,
  });
