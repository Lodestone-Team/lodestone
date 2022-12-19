import { LabelColor } from 'components/Atoms/Label';
import { NextRouter } from 'next/router';
import axios, { AxiosError, AxiosRequestConfig } from 'axios';
import { ClientError } from 'bindings/ClientError';
import { InstanceState } from 'bindings/InstanceState';
import { ClientFile } from 'bindings/ClientFile';
import { QueryClient } from '@tanstack/react-query';
import { Base64 } from 'js-base64';

export const capitalizeFirstLetter = (string: string) => {
  return string.charAt(0).toUpperCase() + string.slice(1);
};

export function round(num: number, precision: number) {
  const factor = Math.pow(10, precision);
  return Math.round(num * factor) / factor;
}

/**
 * Call an async function with a maximum time limit (in milliseconds) for the timeout
 * @param {Promise<any>} asyncPromise An asynchronous promise to resolve
 * @param {number} timeLimit Time limit to attempt function in milliseconds
 * @returns {Promise<any> | undefined} Resolved promise for async function call, or an error if time limit reached
 */
export const asyncCallWithTimeout = async (
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  asyncPromise: Promise<any>,
  timeLimit: number
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
): Promise<any> => {
  let timeoutHandle: NodeJS.Timeout;

  const timeoutPromise = new Promise((_resolve, reject) => {
    timeoutHandle = setTimeout(
      () => reject(new Error('Async call timeout limit reached')),
      timeLimit
    );
  });

  return Promise.race([asyncPromise, timeoutPromise]).then((result) => {
    clearTimeout(timeoutHandle);
    return result;
  });
};

// a map from InstanceStatus to string names
// instancestatus is a union type
export const stateToLabelColor: { [key in InstanceState]: LabelColor } = {
  Running: 'green',
  Starting: 'ochre',
  Stopping: 'ochre',
  Stopped: 'gray',
  Error: 'red',
  // Loading: 'gray',
};

export const pushKeepQuery = (router: NextRouter, pathname: string, shallow = true) => {
  router.push(
    {
      pathname,
      query: router.query,
    },
    undefined,
    { shallow }
  );
};

export const setQuery = (router: NextRouter, key: string, value?: string, pathname?: string) => {
  router.push(
    {
      pathname: pathname || router.pathname,
      query: { ...router.query, [key]: value },
    },
    undefined,
    { shallow: true }
  );
}

export function isAxiosError<ResponseType>(
  error: unknown
): error is AxiosError<ResponseType> {
  return axios.isAxiosError(error);
}

export function errorToString(error: unknown): string {
  if (isAxiosError<any>(error)) {
    if (error.response && error.response.data) {
      let data = error.response.data;
      // if response.data is a string parse it as a JSON object
      if (typeof data === 'string') {
        // spaghetti code
        if (data.startsWith('`Authorization`')) {
          return 'Invalid token: try to log out and log in again';
        }
        data = JSON.parse(data);
      }
      // if response.data is a blob parse it as a JSON object
      if (data instanceof Blob) {
        const reader = new FileReader();
        reader.readAsText(data);
        // reader.onload = () => {
        //   data = JSON.parse(reader.result as string);
        // };
      }
      console.log(data);
      if (data && data.inner) {
        // TODO: more runtime type checking
        const clientError: ClientError = new ClientError(data);
        return clientError.toString();
      } else return `${error.code}: ${error.response.statusText}`;
    } else {
      if (error.code === 'ERR_NETWORK') return 'Network error';
      else return `Network error: ${error.message}`;
    }
  }
  if (error === null) return '';
  if (error instanceof Error) return error.message;
  return `Unknown error`;
}

/**
 * @throws Error
 */
export async function axiosWrapper<ResponseType>(
  config: AxiosRequestConfig
): Promise<ResponseType> {
  try {
    const response = await axios.request<ResponseType>(config);
    return response.data;
  } catch (error) {
    throw new Error(errorToString(error));
  }
}

export async function axiosPutSingleValue<ResponseType>(
  url: string,
  value: unknown
): Promise<ResponseType> {
  return axiosWrapper<ResponseType>({
    method: 'put',
    url,
    data: JSON.stringify(value),
    headers: {
      'Content-Type': 'application/json',
    },
  });
}

// meant to be used with an async function
// that throws an error if the condition is not met
// and returns void otherwise
// returns a string if the condition is not met
// returns empty string otherwise
export async function catchAsyncToString(
  promise: Promise<unknown>
): Promise<string> {
  try {
    await promise;
    return '';
  } catch (e) {
    if (e instanceof Error) return e.message;
    return 'Unknown error';
  }
}

export function parseintStrict(value: string): number {
  // parseint passes even if there are trailing characters
  // so we check that the string is the same as the parsed int
  const parsed = parseInt(value);
  if (parsed.toString() !== value) throw new Error('Not an integer');
  return parsed;
}

export function getWidth(
  el: HTMLElement,
  type: 'inner' | 'outer' | 'width' | 'full'
) {
  if (type === 'inner')
    // .innerWidth()
    return el.clientWidth;
  else if (type === 'outer')
    // .outerWidth()
    return el.offsetWidth;
  const s = window.getComputedStyle(el, null);
  if (type === 'width')
    // .width()
    return (
      el.clientWidth -
      parseInt(s.getPropertyValue('padding-left')) -
      parseInt(s.getPropertyValue('padding-right'))
    );
  else if (type === 'full')
    // .outerWidth(includeMargins = true)
    return (
      el.offsetWidth +
      parseInt(s.getPropertyValue('margin-left')) +
      parseInt(s.getPropertyValue('margin-right'))
    );
  throw new Error('Invalid type');
}

export function getHeight(
  el: HTMLElement,
  type: 'inner' | 'outer' | 'height' | 'full'
) {
  if (type === 'inner')
    // .innerHeight()
    return el.clientHeight;
  else if (type === 'outer')
    // .outerHeight()
    return el.offsetHeight;
  const s = window.getComputedStyle(el, null);
  if (type === 'height')
    // .height()
    return (
      el.clientHeight -
      parseInt(s.getPropertyValue('padding-top')) -
      parseInt(s.getPropertyValue('padding-bottom'))
    );
  else if (type === 'full')
    // .outerHeight(includeMargins = true)
    return (
      el.offsetHeight +
      parseInt(s.getPropertyValue('margin-top')) +
      parseInt(s.getPropertyValue('margin-bottom'))
    );
  throw new Error('Invalid type');
}

// format duration in seconds to DD:HH:MM:SS
export const formatDuration = (duration: number) => {
  const days = Math.floor(duration / 86400);
  const hours = Math.floor((duration % 86400) / 3600);
  const minutes = Math.floor((duration % 3600) / 60);
  const seconds = Math.floor(duration % 60);
  return `${days < 10 ? '0' + days : days}:${
    hours < 10 ? '0' + hours : hours
  }:${minutes < 10 ? '0' + minutes : minutes}:${
    seconds < 10 ? '0' + seconds : seconds
  }`;
};

// format message time for notifications
export const formatNotificationTime = (time_ms: number) => {
  const now = new Date();
  const time = new Date(time_ms);
  if (
    now.getFullYear() === time.getFullYear() &&
    now.getMonth() === time.getMonth() &&
    now.getDate() === time.getDate()
  ) {
    return time.toLocaleTimeString('en-US');
  } else {
    return time.toLocaleString('en-US');
  }
};

// format "time ago"
export const formatTimeAgo = (time_ms: number) => {
  const now = new Date();
  const time = new Date(time_ms);
  const diff = now.getTime() - time.getTime();
  const diffDays = Math.floor(diff / (1000 * 3600 * 24));
  const diffHours = Math.floor(diff / (1000 * 3600));
  const diffMinutes = Math.floor(diff / (1000 * 60));
  const diffSeconds = Math.floor(diff / 1000);
  if (diffDays > 0) {
    return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;
  } else if (diffHours > 0) {
    return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
  } else if (diffMinutes > 0) {
    return `${diffMinutes} minute${diffMinutes > 1 ? 's' : ''} ago`;
  } else if (diffSeconds > 0) {
    return `${diffSeconds} second${diffSeconds > 1 ? 's' : ''} ago`;
  } else {
    return 'just now';
  }
};

// format "progress" number of bytes to human readable string
// using unit appropriate for "total"
export const formatBytesDownload = (
  progress: number,
  total: number,
  decimals = 2
) => {
  if (total === 0) return '0 Bytes';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];
  const i = Math.floor(Math.log(total) / Math.log(k));
  if (i > sizes.length - 1)
    throw new Error('The file is bigger than 1024 YB!? this should not happen');
  return parseFloat((progress / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
};

// format number of bytes to human readable string
export const formatBytes = (bytes: number, decimals = 2) => {
  return formatBytesDownload(bytes, bytes, decimals);
};

export const LODESTONE_EPOCH = BigInt('1667530800000');

// get the timestamp from a snowflake (bitint)
export const getSnowflakeTimestamp = (snowflake_str: string) => {
  const snowflakeBigInt = BigInt(snowflake_str);
  return Number(snowflakeBigInt >> BigInt(22)) + Number(LODESTONE_EPOCH);
};

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
    // TODO: better error display
    alert(error);
    return;
  }
  queryClient.setQueriesData(
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
  queryClient.setQueriesData(fileListKey, newFileList);
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
    // TODO: better error display
    alert(error);
    return;
  }

  const fileListKey = ['instance', uuid, 'fileList', directory];
  const fileList = queryClient.getQueryData<ClientFile[]>(fileListKey);
  if (!fileList) return;
  queryClient.setQueriesData(
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
    // TODO: better error display
    alert(error);
    return;
  }
  const fileListKey = ['instance', uuid, 'fileList', parentDirectory];
  const fileList = queryClient.getQueryData<ClientFile[]>(fileListKey);
  queryClient.setQueriesData(
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
    // TODO: better error display
    alert(error);
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

export const chooseFiles = async () => {
  const input = document.createElement('input');
  input.type = 'file';
  input.multiple = true;
  input.click();
  return new Promise<FileList | null>((resolve) => {
    input.onchange = () => {
      resolve(input.files);
    };
  });
};

// put this in whatever utility library file you have:
export function convertUnicode(input: string) {
  return input
    .replace(/\\+u([0-9a-fA-F]{4})/g, (a, b) =>
      String.fromCharCode(parseInt(b, 16))
    )
    .replace(/\\+n/g, '\n');
}
