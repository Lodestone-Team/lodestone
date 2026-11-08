import { QueryClient, useQuery } from '@tanstack/react-query';
import { ApiKeyInfo } from 'bindings/ApiKeyInfo';
import { ApiKeyScope } from 'bindings/ApiKeyScope';
import { CreateApiKeyRequest } from 'bindings/CreateApiKeyRequest';
import { CreatedApiKey } from 'bindings/CreatedApiKey';
import { PublicUser } from 'bindings/PublicUser';
import { UserPermission } from 'bindings/UserPermission';
import { CoreConnectionInfo } from 'data/LodestoneContext';
import { ApiError, axiosWrapper, errorToString } from 'utils/util';

/*
 * API keys (API_KEYS_SPEC section 3). Requests throw ApiError, like the
 * helpers in utils/apis.ts.
 *
 * The key itself is only in the response of createApiKey. It is never put in
 * the react-query cache: only its ApiKeyInfo is.
 */

export const apiKeyKeys = {
  own: ['api_keys', 'own'],
  ofUser: (uid: string) => ['api_keys', 'user', uid],
};

// a 4xx (such as the 403 for an API key session) won't change on a retry
const retryUnlessClientError = (failureCount: number, error: ApiError) =>
  !(error.status && error.status >= 400 && error.status < 500) &&
  failureCount < 3;

/** The requester's own keys, newest first. */
export const useOwnApiKeys = (enabled = true) =>
  useQuery<ApiKeyInfo[], ApiError>({
    queryKey: apiKeyKeys.own,
    queryFn: ({ signal }) =>
      axiosWrapper<ApiKeyInfo[]>({
        method: 'get',
        url: '/user/api_keys',
        signal,
      }),
    enabled,
    // refetch on every visit, so "last used" is fresh
    staleTime: 0,
    retry: retryUnlessClientError,
  });

/** Another user's keys, for owners and admins. */
export const useUserApiKeys = (uid: string, enabled = true) =>
  useQuery<ApiKeyInfo[], ApiError>({
    queryKey: apiKeyKeys.ofUser(uid),
    queryFn: ({ signal }) =>
      axiosWrapper<ApiKeyInfo[]>({
        method: 'get',
        url: `/user/${encodeURIComponent(uid)}/api_keys`,
        signal,
      }),
    enabled,
    staleTime: 0,
    retry: retryUnlessClientError,
  });

/**
 * Creates a key for the requester. The caller shows the returned key once and
 * then forgets it.
 * @throws ApiError
 */
export const createApiKey = (request: CreateApiKeyRequest) =>
  axiosWrapper<CreatedApiKey>({
    method: 'post',
    url: '/user/api_keys',
    data: request,
  });

/** @throws ApiError */
export const revokeOwnApiKey = (id: string) =>
  axiosWrapper<undefined>({
    method: 'delete',
    url: `/user/api_keys/${encodeURIComponent(id)}`,
  });

/** @throws ApiError */
export const revokeUserApiKey = (uid: string, id: string) =>
  axiosWrapper<undefined>({
    method: 'delete',
    url: `/user/${encodeURIComponent(uid)}/api_keys/${encodeURIComponent(id)}`,
  });

/** Adds a new key's info (never the key) to the requester's list. */
export const addOwnApiKeyToCache = (
  queryClient: QueryClient,
  info: ApiKeyInfo
) => {
  if (!queryClient.getQueryData(apiKeyKeys.own)) {
    queryClient.invalidateQueries({ queryKey: apiKeyKeys.own });
    return;
  }
  queryClient.setQueryData<ApiKeyInfo[]>(apiKeyKeys.own, (old) =>
    old ? [info, ...old.filter((k) => k.id !== info.id)] : old
  );
};

export const removeApiKeyFromCache = (
  queryClient: QueryClient,
  queryKey: string[],
  id: string
) => {
  queryClient.setQueryData<ApiKeyInfo[]>(queryKey, (old) =>
    old ? old.filter((k) => k.id !== id) : old
  );
};

/* Expiry */

export const API_KEY_EXPIRY_CHOICES = [
  { label: '30 days', days: 30 },
  { label: '90 days', days: 90 },
  { label: '1 year', days: 365 },
  { label: 'Never', days: null },
] as const;

export type ApiKeyExpiryLabel =
  (typeof API_KEY_EXPIRY_CHOICES)[number]['label'];

export const DEFAULT_API_KEY_EXPIRY: ApiKeyExpiryLabel = '90 days';

/** The expires_at, in unix seconds, of an expiry choice made at nowMs. */
export const expiresAtFor = (
  label: ApiKeyExpiryLabel,
  nowMs: number = Date.now()
): number | null => {
  const choice = API_KEY_EXPIRY_CHOICES.find((c) => c.label === label);
  if (!choice || choice.days === null) return null;
  return Math.floor(nowMs / 1000) + choice.days * 24 * 60 * 60;
};

export const isApiKeyExpired = (key: ApiKeyInfo, nowMs: number = Date.now()) =>
  key.expires_at !== null && key.expires_at * 1000 <= nowMs;

/* Scope */

export const emptyPermissions = (): UserPermission => ({
  can_view_instance: [],
  can_start_instance: [],
  can_stop_instance: [],
  can_access_instance_console: [],
  can_access_instance_setting: [],
  can_read_instance_resource: [],
  can_write_instance_resource: [],
  can_access_instance_macro: [],
  can_read_instance_file: [],
  can_write_instance_file: [],
  can_create_instance: false,
  can_delete_instance: false,
  can_read_global_file: false,
  can_write_global_file: false,
  can_manage_permission: false,
  can_install_extension: false,
});

/** The number of permissions granted, and of distinct instances they name. */
export const countPermissions = (permissions: UserPermission) => {
  const instances = new Set<string>();
  let granted = 0;
  for (const value of Object.values(permissions)) {
    if (Array.isArray(value)) {
      if (value.length > 0) granted++;
      value.forEach((uuid) => instances.add(uuid));
    } else if (value === true) {
      granted++;
    }
  }
  return { permissions: granted, instances: instances.size };
};

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/**
 * "Full access", "Custom: 3 instances, 5 permissions", or the fixed scope of a
 * Lodestone Launcher device.
 */
export const apiKeyScopeSummary = (scope: ApiKeyScope) => {
  if (scope.type === 'full') return 'Full access';
  if (scope.type === 'launcher') {
    const console =
      scope.console_read && scope.console_write
        ? ', read and use the console'
        : scope.console_read
          ? ', read the console'
          : scope.console_write
            ? ', send console commands'
            : '';
    return `Launcher: view, start and stop instances${console}`;
  }
  const { permissions, instances } = countPermissions(scope.permissions);
  return `Custom: ${plural(instances, 'instance')}, ${plural(
    permissions,
    'permission'
  )}`;
};

// Mirrors User::can_perform_action in the core: admins have these on every
// instance without being granted them.
const ADMIN_IMPLICIT_PERMISSIONS: (keyof UserPermission)[] = [
  'can_view_instance',
  'can_start_instance',
  'can_stop_instance',
  'can_access_instance_console',
  'can_access_instance_setting',
  'can_read_instance_resource',
  'can_read_instance_file',
  'can_create_instance',
  'can_delete_instance',
];

/**
 * Whether user has a permission (on an instance, for per-instance ones), and
 * so can give it to a key with a custom scope.
 */
export const canGrantToApiKey = (
  user: PublicUser,
  permission: keyof UserPermission,
  instanceUuid?: string
): boolean => {
  if (user.is_owner) return true;
  if (user.is_admin && ADMIN_IMPLICIT_PERMISSIONS.includes(permission))
    return true;
  const value = user.permissions[permission];
  if (Array.isArray(value))
    return instanceUuid !== undefined && value.includes(instanceUuid);
  return value === true;
};

/* Errors */

/**
 * The message for a failed API key request. The core refuses API key
 * management to API key sessions with a 403.
 */
export const apiKeyErrorMessage = (error: unknown) => {
  if (
    error instanceof ApiError &&
    error.status === 403 &&
    error.message.includes('signed-in session')
  ) {
    return 'API keys can only be managed when you sign in with your password, not with an API key.';
  }
  return errorToString(error);
};

/* CLI */

/** The core's URL as the CLI takes it, without /api/v1. */
export const cliCoreUrl = (core: CoreConnectionInfo) =>
  `${core.protocol}://${core.address}:${core.port}`;
