import { ClientError } from 'bindings/ClientError';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { UserPermission } from 'bindings/UserPermission';

// this won't ever change. if it does it will be invalidated manually
// an invalid or expired token is logged out by the 401 interceptor in pages/index.tsx
export const useUserInfo = (enabled = true) => {
  const { token } = useContext(LodestoneContext);

  return useQuery<PublicUser, AxiosError<ClientError>>({
    queryKey: ['user', 'info'],
    queryFn: () => {
      return axios
        .get<PublicUser>(`/user/info`)
        .then((response) => response.data);
    },
    enabled: token !== '' && enabled,
  });
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
  const { data: user } = useUserInfo();
  return !!user;
};

/**
 * Parses uid from JWT token directly, might be expired or fake
 */
export const useUid = () => {
  const { uid } = useContext(LodestoneContext);
  return uid;
};
