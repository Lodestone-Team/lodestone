import { useQuery } from '@tanstack/react-query';
import { ClientFile } from '@bindings/ClientFile';
import { Base64 } from 'js-base64';
import { axiosWrapper, fileSorter } from 'utils/util';

export const useFileList = (uuid: string, path: string, enabled: boolean) =>
  useQuery<ClientFile[], Error>(
    ['instance', uuid, 'fileList', path],
    () => {
      return axiosWrapper<ClientFile[]>({
        url: `/instance/${uuid}/fs/${Base64.encode(path, true)}/ls`,
        method: 'GET',
      }).then((response) => {
        // sort by file type, then file name
        return response.sort(fileSorter);
      });
    },
    {
      enabled,
      retry: false,
      cacheTime: 0,
      staleTime: 0,
    }
  );

export const useFileContent = (
  uuid: string,
  file: ClientFile | null,
  enabled: boolean
) =>
  useQuery<string, Error>(
    ['instance', uuid, 'fileContent', file?.path],
    () => {
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
    {
      enabled: file !== null && enabled,
      cacheTime: 0,
      staleTime: 0,
      retry: false,
    }
  );
