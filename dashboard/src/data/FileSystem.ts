import { QueryClient, useQuery } from '@tanstack/react-query';
import { ClientFile } from 'bindings/ClientFile';
import { coreIgnoresCase, nameTaken } from 'components/FileViewer/paths';
import { Base64 } from 'js-base64';
import { axiosWrapper, fileSorter } from 'utils/util';
import type { CoreInfo } from './SystemInfo';

/** The entries of an instance's folder, folders first, then by name. */
export const fetchFileList = (uuid: string, path: string) =>
  axiosWrapper<ClientFile[]>({
    url: `/instance/${uuid}/fs/${Base64.encode(path, true)}/ls`,
    method: 'GET',
  }).then((response) => {
    // sort by file type, then file name
    return response.sort(fileSorter);
  });

/**
 * Whether the folder `path` already has an entry called `name`, so New file
 * and New folder can refuse it instead of emptying an existing file. Lists the
 * folder again rather than trusting the list on screen, which may be old or
 * (from the right-click menu) another folder's. When the folder can't be
 * listed, goes by the list already loaded for it, if any.
 */
export const isNameTaken = async (
  queryClient: QueryClient,
  uuid: string,
  path: string,
  name: string,
  separator: string
) => {
  let entries: ClientFile[];
  try {
    entries = await fetchFileList(uuid, path);
  } catch {
    entries =
      queryClient.getQueryData<ClientFile[]>([
        'instance',
        uuid,
        'fileList',
        path,
      ]) ?? [];
  }
  const os = queryClient.getQueryData<CoreInfo>(['systeminfo', 'CoreInfo'])?.os;
  return nameTaken(entries, name, coreIgnoresCase(os, separator));
};

export const useFileList = (uuid: string, path: string, enabled: boolean) =>
  useQuery<ClientFile[], Error>({
    // Opening a folder keeps the current one on screen until the new listing
    // arrives (it is shown dimmed), instead of blanking the list to "Loading".
    // Only within an instance: another instance's files must never show here.
    placeholderData: (previous, previousQuery) =>
      previousQuery?.queryKey[1] === uuid ? previous : undefined,
    queryKey: ['instance', uuid, 'fileList', path],
    queryFn: () => fetchFileList(uuid, path),
    enabled,
    retry: false,
    gcTime: 0,
    staleTime: 0,
  });

export const useFileContent = (
  uuid: string,
  file: ClientFile | null,
  enabled: boolean
) =>
  useQuery<string, Error>({
    queryKey: ['instance', uuid, 'fileContent', file?.path],
    queryFn: () => {
      return axiosWrapper<string>({
        url: `/instance/${uuid}/fs/${Base64.encode(
          file?.path ?? '',
          true
        )}/read`,
        method: 'GET',
        transformResponse: (data) => data,
      }).then((response) => {
        return response;
      });
    },
    enabled: file !== null && enabled,
    gcTime: 0,
    staleTime: 0,
    retry: false,
  });
