import { ClientError } from 'bindings/ClientError';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { Permission } from 'bindings/Permission';
import { PublicUser } from 'bindings/PublicUser';
import { useContext } from 'react';
import { isAxiosError } from 'utils/util';
import { LodestoneContext } from './LodestoneContext';
import { useCookies } from 'react-cookie';

// this won't ever change. if it does it will be invalidated manually
export const useUserInfo = () => {
  const { token } = useContext(LodestoneContext);
  const [, setCookie] = useCookies(['token']);

  return useQuery<PublicUser, AxiosError<ClientError>>(
    ['user', 'info'],
    () => {
      return axios
        .get<PublicUser>(`/user/info`)
        .then((response) => response.data);
    },
    {
      enabled: useContext(LodestoneContext).isReady && token.length > 0,
      onError: (error) => {
        if (error.response?.data.inner === 'PermissionDenied')
          // then token is invalid, delete it
          setCookie('token', '');
      },
    }
  );
};

export const isUserAuthorized = (
  user: PublicUser | undefined,
  permission: Permission,
  instanceId: string
) => {
  if (!user) return false;
  if (user.is_owner) return true;
  // check if permission in user.permissions, if not return false
  if (!user.permissions[permission]) return false;
  return user.permissions[permission].includes(instanceId);
};

export const useUserAuthorized = (
  permission: Permission,
  instanceId: string
) => {
  const { data: user } = useUserInfo();
  return isUserAuthorized(user, permission, instanceId);
};
