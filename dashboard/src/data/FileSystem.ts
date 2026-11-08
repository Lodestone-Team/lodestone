import { useQuery } from '@tanstack/react-query';
import { ClientFile } from 'bindings/ClientFile';
import { Base64 } from 'js-base64';
import { axiosWrapper, fileSorter } from 'utils/util';

export const useFileList = (uuid: string, path: string, enabled: boolean) =>
  useQuery<ClientFile[], Error>({
    // Opening a folder keeps the current one on screen until the new listing
    // arrives (it is shown dimmed), instead of blanking the list to "Loading".
    // Only within an instance: another instance's files must never show here.
    placeholderData: (previous, previousQuery) =>
      previousQuery?.queryKey[1] === uuid ? previous : undefined,
    queryKey: ['instance', uuid, 'fileList', path],
    queryFn: () => {
      return axiosWrapper<ClientFile[]>({
        url: `/instance/${uuid}/fs/${Base64.encode(path, true)}/ls`,
        method: 'GET',
      }).then((response) => {
        // sort by file type, then file name
        return response.sort(fileSorter);
      });
    },
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
