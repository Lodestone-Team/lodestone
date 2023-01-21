import { QueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { ClientError } from 'bindings/ClientError';
import { ClientFile } from 'bindings/ClientFile';
import { LoginReply } from 'bindings/LoginReply';
import { UserPermission } from 'bindings/UserPermission';
import { Base64 } from 'js-base64';
import { LoginValues } from 'pages/login/UserLogin';
import { toast } from 'react-toastify';
import {
  axiosWrapper,
  catchAsyncToString,
  errorToString,
  getFileName,
  isAxiosError,
  parentPath,
} from './util';

/***********************
 * Start Files API
 ***********************/

export const saveInstanceFile = async (
  uuid: string,
  directory: string,
  file: ClientFile,
  content: string,
  queryClient: QueryClient
) => {
  
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(file.path, true)}/write`,
      data: content,
    })
  );
  if (error) {
    toast.error(error);
    return;
  }
  queryClient.setQueryData(
    ['instance', uuid, 'fileContent', file.path],
    content
  );

  const fileListKey = ['instance', uuid, 'fileList', directory];
  const fileList = queryClient.getQueryData<ClientFile[]>(fileListKey);
  if (!fileList) return;
  const newFileList = fileList.map((f) => {
    if (f.path === file.path)
      return {
        ...f,
        modification_time: Math.round(Date.now() / 1000),
      };
    return f;
  });
  queryClient.setQueryData(fileListKey, newFileList);
};

export const deleteInstanceFile = async (
  uuid: string,
  directory: string,
  file: ClientFile,
  queryClient: QueryClient
) => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'delete',
      url: `/instance/${uuid}/fs/${Base64.encode(file.path, true)}/rm`,
    })
  );
  if (error) {
    toast.error(error);
    return;
  }

  const fileListKey = ['instance', uuid, 'fileList', directory];
  const fileList = queryClient.getQueryData<ClientFile[]>(fileListKey);
  if (!fileList) return;
  queryClient.setQueryData(
    fileListKey,
    fileList?.filter((f) => f.path !== file.path)
  );
};

export const deleteInstanceDirectory = async (
  uuid: string,
  parentDirectory: string,
  directory: string,
  queryClient: QueryClient
) => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'delete',
      url: `/instance/${uuid}/fs/${Base64.encode(directory, true)}/rmdir`,
    })
  );
  if (error) {
    toast.error(error);
    return;
  }
  const fileListKey = ['instance', uuid, 'fileList', parentDirectory];
  const fileList = queryClient.getQueryData<ClientFile[]>(fileListKey);
  queryClient.setQueryData(
    fileListKey,
    fileList?.filter((file) => file.path !== directory)
  );
};

export const downloadInstanceFiles = async (uuid: string, file: ClientFile) => {
  // TODO handle errors
  const tokenResponse = await axiosWrapper<string>({
    method: 'get',
    url: `/instance/${uuid}/fs/${Base64.encode(file.path, true)}/download`,
  });
  const downloadUrl = axios.defaults.baseURL + `/file/${tokenResponse}`;
  window.open(downloadUrl, '_blank');
};

export const uploadInstanceFiles = async (
  uuid: string,
  directory: string,
  file: Array<File>,
  queryClient: QueryClient
) => {
  // upload all files using multipart form data
  const formData = new FormData();
  file.forEach((f) => {
    formData.append('file', f);
  });
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(directory, true)}/upload`,
      data: formData,
      headers: {
        'Content-Type': 'multipart/form-data',
      },
      timeout: 0,
      onUploadProgress: (progressEvent) => {
        console.log(progressEvent);
      },
    })
  );
  if (error) {
    toast.error(error);
    return;
  }

  // invalidate the query instead of updating it because file name might be different
  queryClient.invalidateQueries(['instance', uuid, 'fileList', directory]);
};

export const createInstanceFile = async (
  uuid: string,
  directory: string,
  name: string
) => {
  const filePath = directory + '/' + name;
  return await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(filePath, true)}/new`,
    })
  );
};

export const createInstanceDirectory = async (
  uuid: string,
  parentDirectory: string,
  name: string
) => {
  const filePath = parentDirectory + '/' + name;
  return await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(filePath, true)}/mkdir`,
    })
  );
};

export const moveInstanceFileOrDirectory = async (
  uuid: string,
  source: string,
  destination: string,
  queryClient: QueryClient,
  direcotrySeparator: string
) => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(
        source,
        true
      )}/move/${Base64.encode(destination, true)}`,
    })
  );

  if (error) {
    toast.error(
      `Failed to move ${getFileName(source, direcotrySeparator)}: ${error}`
    );
    return;
  }

  // just invalided the query instead of updating it because file name might be different due to conflict
  queryClient.invalidateQueries([
    'instance',
    uuid,
    'fileList',
    parentPath(source, direcotrySeparator),
  ]);
  queryClient.invalidateQueries([
    'instance',
    uuid,
    'fileList',
    parentPath(destination, direcotrySeparator),
  ]);
};

export const unzipInstanceFile = async (
  uuid: string,
  file: ClientFile,
  targetDirectory: string,
  queryClient: QueryClient,
  direcotrySeparator: string
) => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(
        file.path,
        true
      )}/unzip/${Base64.encode(targetDirectory, true)}`,
    })
  );

  if (error) {
    toast.error(`Failed to unzip ${file.name}: ${error}`);
    return;
  }

  // just invalided the query instead of updating it because file name might be different due to conflict
  queryClient.invalidateQueries([
    'instance',
    uuid,
    'fileList',
    parentPath(file.path, direcotrySeparator),
  ]);
  queryClient.invalidateQueries([
    'instance',
    uuid,
    'fileList',
    targetDirectory,
  ]);
};

/***********************
 * End Files API
 ***********************/

/***********************
 * Start User API
 ***********************/

/**
 * @throws string
 */
export async function loginToCore(
  loginValue: LoginValues
): Promise<LoginReply | undefined> {
  // we manually handle error here because we want to show different error messages
  try {
    return await axios
      .post<LoginReply>(
        '/user/login',
        {},
        {
          auth: {
            username: loginValue.username,
            password: loginValue.password,
          },
        }
      )
      .then((response) => {
        return response.data;
      });
  } catch (error) {
    if (isAxiosError<ClientError>(error) && error.response) {
      if (
        error.response.status === 401 ||
        error.response.status === 403 ||
        error.response.status === 500
      ) {
        throw `Error: ${error.response.data.inner}: ${error.response.data.detail}`;
      }
    } else {
      throw `Login failed: ${errorToString(error)}`;
    }
  }
}

/**
 * @throws string if error
 * @returns LoginReply if success
 */
export const createNewUser = async (values: {
  username: string;
  password: string;
}) => {
  return await axiosWrapper<LoginReply>({
    method: 'post',
    url: '/user',
    data: values,
  });
};

/**
 * @throws string if error
 * @returns undefined if success
 */
export const changePassword = async (values: {
  uid: string;
  old_password: string | null;
  new_password: string;
}) => {
  return await axiosWrapper<undefined>({
    method: 'put',
    url: `/user/${values.uid}/password`,
    data: values,
  });
};

/**
 * @throws string if error
 * @returns undefined if success
 */
export const deleteUser = async (uid: string) => {
  return await axiosWrapper<undefined>({
    method: 'delete',
    url: `/user/${uid}`,
  });
};

/**
 * @throws string if error
 * @returns undefined if success
 * @param uid user id
 */
export const changeUserPermissions = async (
  uid: string,
  permission: UserPermission
) => {
  return await axiosWrapper<undefined>({
    method: 'put',
    url: `/user/${uid}/update_perm`,
    data: permission,
  });
};

/***********************
 * End User API
 ***********************/

export const openPort = async (port: number) => {
  return await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/gateway/open_port/${port}`,
    })
  );
};
