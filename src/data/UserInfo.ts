import { ClientError } from 'bindings/ClientError';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { UserPermission } from 'bindings/UserPermission';

// this won't ever change. if it does it will be invalidated manually
export const useUserInfo = () => {
  const { token, setToken, socket } = useContext(LodestoneContext);

  return useQuery<PublicUser, AxiosError<ClientError>>(
    ['user', 'info'],
    () => {
      return axios
        .get<PublicUser>(`/user/info`)
        .then((response) => response.data);
    },
    {
      enabled: useContext(LodestoneContext).isReady && token !== '',
      onError: (error) => {
        if (error.response?.data?.inner === 'Unauthorized')
          // then token is invalid, delete it
          setToken('', socket);
      },
    }
  );
};

// check if type is boolean
const isPermissionGlobal = (permission: unknown): permission is boolean => {
  return typeof permission === 'boolean';
};

// check if type is array
const isPermissionArray = (permission: unknown): permission is string[] => {
  return Array.isArray(permission);
};

export const isUserAuthorized = (
  user: PublicUser | undefined,
  permission: keyof UserPermission,
  instanceId?: string
) => {
  if (!user) return false;
  if (user.is_owner) return true;
  // check if permission in user.permissions, if not return false
  if (!user.permissions[permission]) return false;

  const permissionValue = user.permissions[permission];
  if (isPermissionGlobal(permissionValue)) {
    return permissionValue;
  } else if (isPermissionArray(permissionValue)) {
    if (!instanceId)
      throw new Error(`instanceId is required for ${permission}`);
    return permissionValue.includes(instanceId);
  }
  return false;
};

export const useUserAuthorized = (
  permission: keyof UserPermission,
  instanceId?: string
) => {
  const { data: user } = useUserInfo();
  return isUserAuthorized(user, permission, instanceId);
};

export const useUserLoggedIn = () => {
  const {data: user} = useUserInfo();
  return !!user;
}
