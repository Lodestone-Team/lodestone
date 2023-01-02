import jwt from 'jsonwebtoken';
import { ClientError } from 'bindings/ClientError';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import { useContext, useMemo } from 'react';
import { LodestoneContext } from './LodestoneContext';
import { UserPermission } from 'bindings/UserPermission';
import { errorToString } from 'utils/util';

// this won't ever change. if it does it will be invalidated manually
export const useUserInfo = (enabled = true) => {
  const { token, setToken, core } = useContext(LodestoneContext);
  const { port, address } = core;
  const socket = `${address}:${port}`;

  return useQuery<PublicUser, AxiosError<ClientError>>(
    ['user', 'info'],
    () => {
      return axios
        .get<PublicUser>(`/user/info`)
        .then((response) => response.data);
    },
    {
      enabled: token !== '' && enabled,
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
  const { data: user } = useUserInfo();
  return !!user;
};

/**
 * Never throws
 * @returns JWT payload or undefined
 */
export const useDecodedToken = () => {
  const { token } = useContext(LodestoneContext);
  const decoded = useMemo(() => {
    try {
      const decoded = jwt.decode(token, { complete: true });
      if (!decoded) throw new Error('Invalid token');
      return decoded.payload;
    } catch (e) {
      const message = errorToString(e);
      console.error(message);
    }
  }, [token]);
  return decoded;
};

/**
 * Parses uid from JWT token directly, might be expired or fake
 * Don't use for high security stuff
 * @returns uid or undefined
 */
export const useUid = () => {
  const decoded = useDecodedToken();
  if (typeof decoded === 'undefined') return undefined;
  if (typeof decoded === 'string') return undefined;
  return decoded?.uid;
};
