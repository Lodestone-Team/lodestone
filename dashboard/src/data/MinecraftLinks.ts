import { useQuery } from '@tanstack/react-query';
import { MinecraftLink } from 'bindings/MinecraftLink';
import { MinecraftLinkInfo } from 'bindings/MinecraftLinkInfo';
import { useMemo } from 'react';
import { useUserInfo } from 'data/UserInfo';
import { ApiError, axiosWrapper } from 'utils/util';
import { LinkedPlayers, linkedPlayers } from 'utils/linkedPlayers';

/*
 * Minecraft accounts linked to Lodestone users. Links are made in Lodestone
 * Launcher, which proves the account is the user's; here they are listed and
 * removed. Everyone sees their own links; the owner sees everyone's.
 */

export const minecraftLinkKeys = {
  own: ['minecraft_links', 'own'],
  all: ['minecraft_links', 'all'],
};

// an older core answers 404; nothing to retry
const noRetryOnClientError = (failureCount: number, error: ApiError) =>
  !(error.status && error.status >= 400 && error.status < 500) &&
  failureCount < 3;

export const useOwnMinecraftLinks = (enabled = true) =>
  useQuery<MinecraftLink[], ApiError>({
    queryKey: minecraftLinkKeys.own,
    queryFn: ({ signal }) =>
      axiosWrapper<MinecraftLink[]>({
        method: 'get',
        url: '/user/minecraft_links',
        signal,
      }),
    enabled,
    retry: noRetryOnClientError,
  });

/** Every user's links, for the owner. */
export const useAllMinecraftLinks = (enabled = true) =>
  useQuery<MinecraftLinkInfo[], ApiError>({
    queryKey: minecraftLinkKeys.all,
    queryFn: ({ signal }) =>
      axiosWrapper<MinecraftLinkInfo[]>({
        method: 'get',
        url: '/minecraft_links',
        signal,
      }),
    enabled,
    retry: noRetryOnClientError,
  });

/** Removes one of the requester's links, or (`anyUser`, the owner) anyone's. */
export const unlinkMinecraftAccount = (
  minecraftUuid: string,
  anyUser: boolean
) =>
  axiosWrapper<null>({
    method: 'delete',
    url: `${anyUser ? '/minecraft_links' : '/user/minecraft_links'}/${encodeURIComponent(minecraftUuid)}`,
  });

/**
 * The Lodestone users of linked players, by Minecraft name and UUID: every
 * link for the owner, the user's own links for everyone else.
 */
export const useLinkedPlayers = (): LinkedPlayers => {
  const { data: user } = useUserInfo();
  const isOwner = !!user?.is_owner;
  const { data: all } = useAllMinecraftLinks(isOwner);
  const { data: own } = useOwnMinecraftLinks(!!user && !isOwner);
  return useMemo(
    () =>
      linkedPlayers(
        isOwner
          ? (all ?? [])
          : (own ?? []).map((link) => ({
              ...link,
              username: user?.username ?? '',
            }))
      ),
    [isOwner, all, own, user?.username]
  );
};
