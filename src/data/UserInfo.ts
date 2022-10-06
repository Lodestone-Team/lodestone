import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { Permission } from 'bindings/Permission';
import { PublicUser } from 'bindings/PublicUser';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';

export const useUserInfo = () => {
  return useQuery<PublicUser, AxiosError>(
    ['user', 'info'],
    () => {
      return axios
        .get<PublicUser>(`/user/info`)
        .then((response) => response.data);
    },
    {
      enabled: useContext(LodestoneContext).isReady,
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
