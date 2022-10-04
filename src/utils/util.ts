import { Result } from '@badrap/result';
import { ClientError } from 'data/ClientError';
import { InstanceState } from 'data/InstanceList';
import { LabelColor } from 'components/Label';
import { NextRouter } from 'next/router';
import axios, { AxiosError, AxiosRequestConfig } from 'axios';

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

export async function axiosWrapper<ResponseType>(
  config: AxiosRequestConfig
): Promise<Result<ResponseType, ClientError>> {
  try {
    const response = await axios.request<ResponseType>(config);
    return Result.ok(response.data);
  } catch (error) {
    if (isAxiosError<ClientError>(error)) {
      if (error.response) {
        if (error.response.data.inner) {
          // TODO: more runtime type checking
          return Result.err(error.response.data);
        } else
          return Result.err(
            new ClientError('NetworkError', `${error.code}: ${error.response.statusText}`)
          );
      } else return Result.err(new ClientError('NetworkError', error.message));
    }
    return Result.err(new ClientError('UnknownError', 'Unknown error'));
  }
}

export async function axiosPutSingleValue<ResponseType>(
  url: string,
  value: unknown
): Promise<Result<ResponseType, ClientError>> {
  return axiosWrapper<ResponseType>({
    method: 'put',
    url,
    data: JSON.stringify(value),
    headers: {
      'Content-Type': 'application/json',
    }
  });
}
