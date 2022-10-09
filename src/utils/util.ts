import { LabelColor } from 'components/Atoms/Label';
import { NextRouter } from 'next/router';
import axios, { AxiosError, AxiosRequestConfig } from 'axios';
import { ClientError } from 'bindings/ClientError';
import { InstanceState } from 'bindings/InstanceState';

export const capitalizeFirstLetter = (string: string) => {
  return string.charAt(0).toUpperCase() + string.slice(1);
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

export const pushKeepQuery = (router: NextRouter, pathname: string) => {
  router.push(
    {
      pathname,
      query: router.query,
    },
    undefined,
    { shallow: true }
  );
};

export function isAxiosError<ResponseType>(
  error: unknown
): error is AxiosError<ResponseType> {
  return axios.isAxiosError(error);
}

export function errorToMessage(error: unknown): string {
  if (isAxiosError<ClientError>(error)) {
    if (error.response) {
      if (error.response.data && error.response.data.inner) {
        // TODO: more runtime type checking
        return error.response.data.toString();
      } else if (error.code === 'ERR_NETWORK') {
        return 'Network error';
      } else return `${error.code}: ${error.response.statusText}`;
    } else return `Network error: ${error.code}`;
  }
  if (error === null) return '';
  return `Unknown error: ${error}`;
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
    throw new Error(errorToMessage(error));
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
