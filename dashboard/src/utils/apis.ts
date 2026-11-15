import { QueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { ClientFile } from 'bindings/ClientFile';
import { LoginReply } from 'bindings/LoginReply';
import { PublicUser } from 'bindings/PublicUser';
import { UserPermission } from 'bindings/UserPermission';
import { Base64 } from 'js-base64';
import { toast } from 'react-toastify';
import {
  ApiError,
  axiosWrapper,
  catchAsyncToString,
  errorToString,
  getFileName,
  isAxiosError,
  joinPath,
  parentPath,
  toApiError,
} from './util';
import { UnzipOption } from 'bindings/UnzipOption';
import { CopyInstanceFileRequest } from 'bindings/CopyInstanceFileRequest';
import { ZipRequest } from 'bindings/ZipRequest';
import { t } from 'i18n';
import { basicAuthHeader } from './basicAuth';
import { startDownload } from './download';
import { tauri } from './tauriUtil';

/*
 * Error convention
 *
 * Every request goes through axiosWrapper, which throws an ApiError (an Error
 * with a message formatted for the user, plus the HTTP status and error kind).
 * (loginToCore calls axios itself to read the Retry-After header, and also
 * throws ApiError.)
 * Helpers that return data resolve with it and throw ApiError on failure.
 *
 * Older helpers keep the shape the components depend on, and never throw:
 * - marked "Returns the error message": resolve with '' on success or the
 *   error message on failure (catchAsyncToString); always check the result.
 * - marked "Shows a toast on failure": report the error to the user
 *   themselves and resolve with nothing.
 */

export type LoginValues = {
  username: string;
  password: string;
};

/***********************
 * Start Files API
 ***********************/

/** Shows a toast on failure. */
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
        size: content.length,
      };
    return f;
  });
  queryClient.setQueryData(fileListKey, newFileList);
};

/**
 * Shows a toast on failure (and on success). Resolves with whether the file
 * was deleted.
 */
export const deleteInstanceFile = async (
  uuid: string,
  directory: string,
  file: ClientFile,
  queryClient: QueryClient
): Promise<boolean> => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'delete',
      url: `/instance/${uuid}/fs/${Base64.encode(file.path, true)}/rm`,
    })
  );
  if (error) {
    toast.error(error);
    return false;
  } else {
    toast.success(t('server.api.deletedFile', { name: file.name }));
  }

  const fileListKey = ['instance', uuid, 'fileList', directory];
  const fileList = queryClient.getQueryData<ClientFile[]>(fileListKey);
  if (!fileList) return true;
  queryClient.setQueryData(
    fileListKey,
    fileList?.filter((f) => f.path !== file.path)
  );
  return true;
};

/**
 * Shows a toast on failure (and on success). Resolves with whether the folder
 * was deleted.
 */
export const deleteInstanceDirectory = async (
  uuid: string,
  parentDirectory: string,
  directory: string,
  queryClient: QueryClient
): Promise<boolean> => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'delete',
      url: `/instance/${uuid}/fs/${Base64.encode(directory, true)}/rmdir`,
    })
  );
  if (error) {
    toast.error(error);
    return false;
  } else {
    toast.success(t('server.api.deletedDirectory', { path: directory }));
  }
  const fileListKey = ['instance', uuid, 'fileList', parentDirectory];
  const fileList = queryClient.getQueryData<ClientFile[]>(fileListKey);
  queryClient.setQueryData(
    fileListKey,
    fileList?.filter((file) => file.path !== directory)
  );
  return true;
};

/**
 * The URL that downloads a file the core registered under `key` (see
 * `/file/{key}` in the core's global_fs.rs). Start it with downloadFromCore.
 */
export const fileDownloadUrl = (key: string) =>
  axios.defaults.baseURL + `/file/${key}`;

/**
 * Starts downloading a fileDownloadUrl link, in browsers and in Lodestone Host
 * (where the system browser downloads it). See startDownload: no popup, so it
 * still works after awaiting the link.
 * @throws if Lodestone Host could not hand the link to the browser
 */
export const downloadFromCore = (url: string) => {
  const host = tauri;
  return startDownload(
    url,
    document,
    host
      ? (link) => host.core.invoke('plugin:opener|open_url', { url: link })
      : undefined
  );
};

/** @throws ApiError */
export const requestInstanceFileUrl = async (
  uuid: string,
  file: ClientFile,
  timeout = 5000
): Promise<string> => {
  const tokenResponse = await axiosWrapper<string>({
    method: 'get',
    url: `/instance/${uuid}/fs/${Base64.encode(file.path, true)}/url`,
    timeout: timeout,
  });
  return fileDownloadUrl(tokenResponse);
};

/**
 * Shows a toast on failure. Resolves with whether the download started.
 */
export const downloadInstanceFile = async (
  uuid: string,
  file: ClientFile,
  timeout = 5000
): Promise<boolean> => {
  try {
    const url = await requestInstanceFileUrl(uuid, file, timeout);
    await downloadFromCore(url);
    return true;
  } catch (error) {
    toast.error(
      t('server.api.downloadFailed', {
        name: file.name,
        error: errorToString(error),
      })
    );
    return false;
  }
};

/** Shows a toast on failure. */
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
  toast.info(t('server.api.uploading', { count: file.length }));
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(directory, true)}/upload`,
      data: formData,
      headers: {
        'Content-Type': 'multipart/form-data',
      },
      timeout: 0,
    })
  );
  if (error) {
    toast.error(error);
    return;
  }

  // invalidate the query instead of updating it because file name might be different
  queryClient.invalidateQueries({
    queryKey: ['instance', uuid, 'fileList', directory],
  });
};

/**
 * Returns the error message.
 * @param direcotrySeparator the core's directory separator ('\\' on Windows)
 */
export const createInstanceFile = async (
  uuid: string,
  directory: string,
  name: string,
  direcotrySeparator = '/'
) => {
  const filePath = joinPath(directory, name, direcotrySeparator);
  return await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(filePath, true)}/new`,
    })
  );
};

/**
 * Returns the error message.
 * @param direcotrySeparator the core's directory separator ('\\' on Windows)
 */
export const createInstanceDirectory = async (
  uuid: string,
  parentDirectory: string,
  name: string,
  direcotrySeparator = '/'
) => {
  const filePath = joinPath(parentDirectory, name, direcotrySeparator);
  return await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(filePath, true)}/mkdir`,
    })
  );
};

/** Shows a toast on failure. */
export const copyRecursive = async (
  uuid: string,
  request: CopyInstanceFileRequest,
  direcotrySeparator: string,
  queryClient: QueryClient
) => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/cpr`,
      data: request,
    })
  );
  if (error) {
    toast.error(error);
    return;
  }
  queryClient.invalidateQueries({
    queryKey: [
      'instance',
      uuid,
      'fileList',
      parentPath(request.relative_path_dest, direcotrySeparator),
    ],
  });
};

/**
 * Shows a toast on failure, and returns the error message (or undefined on
 * success).
 */
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
      t('server.api.moveFailed', {
        name: getFileName(source, direcotrySeparator),
        error,
      })
    );
    return error;
  }

  // just invalided the query instead of updating it because file name might be different due to conflict
  queryClient.invalidateQueries({
    queryKey: [
      'instance',
      uuid,
      'fileList',
      parentPath(source, direcotrySeparator),
    ],
  });
  queryClient.invalidateQueries({
    queryKey: [
      'instance',
      uuid,
      'fileList',
      parentPath(destination, direcotrySeparator),
    ],
  });
};

/** Shows a toast on failure. */
export const zipInstanceFiles = async (
  uuid: string,
  zipRequest: ZipRequest
) => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/zip`,
      data: zipRequest,
      headers: {
        'Content-Type': 'application/json',
      },
    })
  );
  if (error) {
    toast.error(t('server.api.zipFailed', { error }));
    return;
  } else {
    toast.info(
      t('server.api.zipping', {
        count: zipRequest.target_relative_paths.length,
      })
    );
  }
};

/** Shows a toast on failure. */
export const unzipInstanceFile = async (
  uuid: string,
  file: ClientFile,
  unzipOption: UnzipOption
) => {
  const error = await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/instance/${uuid}/fs/${Base64.encode(file.path, true)}/unzip`,
      data: unzipOption,
      headers: {
        'Content-Type': 'application/json',
      },
    })
  );

  if (error) {
    toast.error(t('server.api.unzipFailed', { name: file.name, error }));
    return;
  } else {
    toast.info(t('server.api.unzipping', { name: file.name }));
  }
};

/***********************
 * End Files API
 ***********************/

/***********************
 * Start User API
 ***********************/

/** The message for a login the core refused with 429 Too Many Requests. */
function tooManyLoginAttemptsMessage(retryAfter: unknown): string {
  // Retry-After is a number of seconds (or an HTTP date, not used here)
  const seconds = Number(retryAfter);
  if (!Number.isFinite(seconds) || seconds <= 0)
    return t('server.api.tooManyAttempts');
  return seconds < 120
    ? t('server.api.tooManyAttemptsSeconds', { count: Math.ceil(seconds) })
    : t('server.api.tooManyAttemptsMinutes', {
        count: Math.ceil(seconds / 60),
      });
}

/**
 * Logs in with basic auth.
 * @throws ApiError: "Error: <reason>" if the core answered with an error,
 *   "Login failed: <reason>" if the core could not be reached, or a
 *   "Too many failed sign-in attempts" message (status 429)
 */
export async function loginToCore(
  loginValue: LoginValues
): Promise<LoginReply> {
  try {
    const response = await axios.request<LoginReply>({
      method: 'post',
      url: '/user/login',
      data: {},
      // not axios's `auth`, which sends the username as Latin-1
      headers: {
        Authorization: basicAuthHeader(
          loginValue.username,
          loginValue.password
        ),
      },
    });
    return response.data;
  } catch (error) {
    const apiError = toApiError(error);
    if (isAxiosError(error) && error.response?.status === 429)
      throw new ApiError(
        tooManyLoginAttemptsMessage(error.response.headers['retry-after']),
        429,
        apiError.kind
      );
    if (apiError.status === undefined)
      throw new ApiError(
        t('server.api.loginFailed', { message: apiError.message })
      );
    throw new ApiError(
      t('server.api.loginError', { message: apiError.message }),
      apiError.status,
      apiError.kind
    );
  }
}

/**
 * Checks an API key (`lsk_…`) for signing in with it: the key is then the session's
 * token. Resolves with the user the key acts as, with the key's scope.
 * @throws ApiError
 */
export async function loginWithApiKey(key: string): Promise<PublicUser> {
  try {
    const response = await axios.post<{ user: PublicUser }>(
      '/user/login_api_key',
      { key }
    );
    return response.data.user;
  } catch (error) {
    const apiError = toApiError(error);
    if (isAxiosError(error) && error.response?.status === 429)
      throw new ApiError(
        tooManyLoginAttemptsMessage(error.response.headers['retry-after']),
        429,
        apiError.kind
      );
    if (apiError.status === undefined)
      throw new ApiError(
        t('server.api.loginFailed', { message: apiError.message })
      );
    throw new ApiError(
      t('server.api.loginError', { message: apiError.message }),
      apiError.status,
      apiError.kind
    );
  }
}

/**
 * @throws ApiError
 * @returns the new user if success
 */
export const createNewUser = async (values: {
  username: string;
  password: string;
}) => {
  return await axiosWrapper<PublicUser>({
    method: 'post',
    url: '/user',
    data: values,
  });
};

/**
 * @throws ApiError
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
 * @throws ApiError
 * @returns undefined if success
 */
export const deleteUser = async (uid: string) => {
  return await axiosWrapper<undefined>({
    method: 'delete',
    url: `/user/${uid}`,
  });
};

/**
 * @throws ApiError
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

/** Returns the error message. */
export const openPort = async (port: number) => {
  return await catchAsyncToString(
    axiosWrapper<null>({
      method: 'put',
      url: `/gateway/open_port/${port}`,
    })
  );
};
