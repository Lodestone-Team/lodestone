import { CoreConnectionInfo } from 'data/LodestoneContext';
import { isEdge } from 'react-device-detect';
import { LabelColor } from 'components/Atoms/Label';
import axios, { AxiosError, AxiosRequestConfig } from 'axios';
import { ClientError } from 'bindings/ClientError';
import { InstanceState } from 'bindings/InstanceState';
import { ClientFile } from 'bindings/ClientFile';
import React from 'react';
import clsx, { ClassValue } from 'clsx';
import {
  formatBytes as formatBytesIntl,
  formatDate,
  formatDateTime,
  formatRelativeTime,
  t,
} from 'i18n';
// eslint-disable-next-line no-restricted-imports
import { extendTailwindMerge } from 'tailwind-merge';

export const DISABLE_AUTOFILL = isEdge
  ? 'off-random-string-edge-stop-ignoring-autofill-off'
  : 'off';
export const LODESTONE_PORT = 16662;
export const DEFAULT_LOCAL_CORE: CoreConnectionInfo = {
  address: 'localhost',
  port: LODESTONE_PORT.toString(),
  protocol: 'http',
  apiVersion: 'v1',
};
// custom font sizes from tailwind.config.js (text-* is a color otherwise)
export const myTwMerge = extendTailwindMerge({
  extend: {
    theme: {
      text: [
        'caption',
        'small',
        'medium',
        'h3',
        'h2',
        'h1',
        'title',
        '2xlarge',
        '3xlarge',
        '4xlarge',
        '5xlarge',
        '6xlarge',
      ],
    },
  },
});

export const supportedZip = ['zip', 'gz', 'tgz'];

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
  Starting: 'yellow',
  Stopping: 'yellow',
  Stopped: 'gray',
  Error: 'red',
  // Loading: 'gray',
};

export const stateToColor: { [key in InstanceState]: string } = {
  Starting: 'text-yellow-300',
  Running: 'text-green-300',
  Stopping: 'text-yellow-300',
  Stopped: 'text-gray-faded/30',
  Error: 'text-red-200',
};

export function isAxiosError<ResponseType>(
  error: unknown
): error is AxiosError<ResponseType> {
  return axios.isAxiosError(error);
}

/**
 * The error that every API helper in utils/apis.ts throws (through
 * axiosWrapper). The message is already formatted for the user. status is the
 * HTTP status, or undefined for network errors and timeouts.
 */
export class ApiError extends Error {
  status: number | undefined;
  kind: ClientError['kind'];

  constructor(message: string, status?: number, kind?: ClientError['kind']) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.kind = kind;
  }
}

// a response body can be a ClientError object, a JSON string of one, plain
// text (e.g. a proxy's HTML error page) or a Blob (download requests)
function parseErrorBody(data: unknown): unknown {
  if (typeof data !== 'string') return data;
  try {
    return JSON.parse(data);
  } catch {
    return data;
  }
}

function isClientErrorBody(data: unknown): data is Partial<ClientError> {
  return typeof data === 'object' && data !== null && 'kind' in data;
}

/**
 * Formats any thrown value as a message for the user. Never throws.
 */
export function errorToString(error: unknown): string {
  if (error instanceof ApiError) return error.message;
  if (isAxiosError<unknown>(error)) {
    if (error.response) {
      const data = parseErrorBody(error.response.data);
      if (typeof data === 'string' && data.startsWith('`Authorization`')) {
        return t('common.invalidToken');
      }
      if (isClientErrorBody(data)) return new ClientError(data).toString();
      return `${error.code}: ${error.response.statusText}`;
    }
    if (error.code === 'ERR_NETWORK') return t('common.networkError');
    return t('common.networkErrorWithMessage', { message: error.message });
  }
  if (error === null || error === undefined) return '';
  if (error instanceof Error) return error.message;
  // if it's string
  if (typeof error === 'string') return error;
  return t('common.unknownError');
}

/**
 * Converts any thrown value to an ApiError, keeping the HTTP status and the
 * error kind of axios errors.
 */
export function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  if (isAxiosError<unknown>(error)) {
    const data = parseErrorBody(error.response?.data);
    return new ApiError(
      errorToString(error),
      error.response?.status,
      isClientErrorBody(data) ? data.kind : undefined
    );
  }
  return new ApiError(errorToString(error));
}

/**
 * The error convention for API helpers: resolves with the response data, or
 * throws an ApiError whose message is formatted for the user.
 * @throws ApiError
 */
export async function axiosWrapper<ResponseType>(
  config: AxiosRequestConfig
): Promise<ResponseType> {
  try {
    const response = await axios.request<ResponseType>(config);
    return response.data;
  } catch (error) {
    throw toApiError(error);
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

/**
 * Awaits a promise and returns '' if it resolved, or the formatted error
 * message if it rejected. Never throws.
 */
export async function catchAsyncToString(
  promise: Promise<unknown>
): Promise<string> {
  try {
    await promise;
    return '';
  } catch (e) {
    return errorToString(e) || t('common.unknownError');
  }
}

export function parseintStrict(value: string): number {
  // parseint passes even if there are trailing characters
  // so we check that the string is the same as the parsed int
  const parsed = parseInt(value);
  if (parsed.toString() !== value) throw new Error(t('common.notAnInteger'));
  return parsed;
}

export function parseFloatStrict(value: string): number {
  // parseFloat passes even if there are trailing characters
  // so we check that the string is the same as the parsed float
  const parsed = parseFloat(value);
  if (parsed.toString() !== value) throw new Error(t('common.notANumber'));
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

// format message time for notifications: the time today, else date and time
export const formatNotificationTime = (time_ms: number) => {
  const now = new Date();
  const time = new Date(time_ms);
  if (
    now.getFullYear() === time.getFullYear() &&
    now.getMonth() === time.getMonth() &&
    now.getDate() === time.getDate()
  ) {
    return formatDate(time, { timeStyle: 'medium' });
  } else {
    return formatDateTime(time);
  }
};

// format "time ago", in the dashboard language: "3 minutes ago" within a
// day, the date before that
export const formatTimeAgo = (time_ms: number) => {
  if (Date.now() - time_ms >= 24 * 3600 * 1000)
    return formatDate(time_ms, { dateStyle: 'long' });
  return formatRelativeTime(time_ms);
};

// format "progress" number of bytes to human readable string
// using unit appropriate for "total"
export const formatBytesDownload = (
  progress: number,
  total: number,
  decimals = 2
) => formatBytesIntl(progress, decimals, total);

// format number of bytes to human readable string, e.g. "1.5 MB"
export const formatBytes = (bytes: number, decimals = 2) =>
  formatBytesIntl(bytes, decimals);

export const LODESTONE_EPOCH = BigInt('1667530800000');

// get the timestamp from a snowflake (bitint)
export const getSnowflakeTimestamp = (snowflake: string) => {
  const snowflakeBigInt = BigInt(snowflake);
  return Number(snowflakeBigInt >> BigInt(22)) + Number(LODESTONE_EPOCH);
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

// use combined refs to merge multiple refs into one
// could take React.RefObject<T> or React.ForwardedRef<T>
export function useCombinedRefs<T>(...refs: any[]) {
  const targetRef = React.useRef<T>(null);

  React.useEffect(() => {
    refs.forEach((ref: any) => {
      if (!ref) return;

      if (typeof ref === 'function') {
        ref(targetRef.current);
      } else {
        ref.current = targetRef.current;
      }
    });
  }, [refs]);

  return targetRef;
}

export const parentPath = (path: string, direcotrySeparator: string) => {
  const pathParts = path.split(direcotrySeparator);
  pathParts.pop();
  return pathParts.join(direcotrySeparator) || '.';
};

export const getFileName = (path: string, direcotrySeparator: string) => {
  const pathParts = path.split(direcotrySeparator);
  return pathParts[pathParts.length - 1];
};

// check if a core is localhost
// IPv6 addresses are stored with brackets (see CoreConnect), e.g. [::1]
export function isLocalCore(core: CoreConnectionInfo) {
  const address = core.address.replace(/^\[(.*)\]$/, '$1');
  return (
    address === 'localhost' ||
    address === '127.0.0.1' ||
    address === '::1' ||
    /^0+:0+:0+:0+:0+:0+:0+:1$/.test(address)
  );
}

// the REST base URL of a core, e.g. http://localhost:16662/api/v1
export function coreApiBaseUrl(core: CoreConnectionInfo) {
  return `${core.protocol}://${core.address}:${core.port}/api/${core.apiVersion}`;
}

// the WebSocket URL of a core endpoint, e.g. ws://localhost:16662/api/v1/events/all/stream
// path starts with a slash and may include a query string
export function coreWebSocketUrl(core: CoreConnectionInfo, path: string) {
  const protocol = core.protocol === 'https' ? 'wss' : 'ws';
  return `${protocol}://${core.address}:${core.port}/api/${core.apiVersion}${path}`;
}

/**
 * Joins a directory and a name with the core's directory separator,
 * e.g. joinPath('.', 'a', '\\') is '.\a'.
 */
export const joinPath = (
  directory: string,
  name: string,
  direcotrySeparator: string
) => {
  if (directory === '') return name;
  if (directory.endsWith(direcotrySeparator)) return `${directory}${name}`;
  return `${directory}${direcotrySeparator}${name}`;
};

export function cn(...inputs: ClassValue[]) {
  return myTwMerge(clsx(...inputs));
}

// detect if we are in a browser that supports negative lookbehind
export const negativeLookbehindSupported = (() => {
  try {
    new RegExp('(?<!\\w)foo');
    return true;
  } catch (e) {
    return false;
  }
})();

export const fileSorter = (a: ClientFile, b: ClientFile) => {
  if (a.file_type === b.file_type) {
    return a.name.localeCompare(b.name);
  }
  return a.file_type.localeCompare(b.file_type);
};
