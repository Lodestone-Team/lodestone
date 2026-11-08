import type { Decorator, Meta, StoryObj } from '@storybook/nextjs-vite';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import axios, {
  AxiosError,
  AxiosResponse,
  InternalAxiosRequestConfig,
} from 'axios';
import { ApiKeyInfo } from 'bindings/ApiKeyInfo';
import { CreateApiKeyRequest } from 'bindings/CreateApiKeyRequest';
import { CreatedApiKey } from 'bindings/CreatedApiKey';
import { PublicUser } from 'bindings/PublicUser';
import { emptyPermissions } from 'data/ApiKeys';
import { LodestoneContext } from 'data/LodestoneContext';
import { ReactNode, useLayoutEffect } from 'react';
import ApiKeysSection from './ApiKeysSection';

const now = Math.floor(Date.now() / 1000);
const day = 24 * 60 * 60;

const owner: PublicUser = {
  uid: 'owner-uid',
  username: 'owner',
  is_owner: true,
  is_admin: false,
  permissions: emptyPermissions(),
};

// can see both instances, but only start the first one
const member: PublicUser = {
  uid: 'member-uid',
  username: 'member',
  is_owner: false,
  is_admin: false,
  permissions: {
    ...emptyPermissions(),
    can_view_instance: ['instance-1', 'instance-2'],
    can_start_instance: ['instance-1'],
    can_create_instance: true,
  },
};

const instances = [
  { uuid: 'instance-1', name: 'Survival' },
  { uuid: 'instance-2', name: 'Creative' },
];

const keys: ApiKeyInfo[] = [
  {
    id: 'a1b2c3d4e5f6',
    name: 'CI deploy',
    display: 'lsk_a1b2c3d4e5f6_…',
    uid: owner.uid,
    created_at: now - 3 * day,
    expires_at: now + 87 * day,
    last_used_at: now - 3600,
    scope: {
      type: 'custom',
      permissions: {
        ...emptyPermissions(),
        can_view_instance: ['instance-1', 'instance-2'],
        can_start_instance: ['instance-1'],
        can_stop_instance: ['instance-1'],
        can_create_instance: true,
      },
    },
  },
  {
    id: 'z9y8x7w6v5u4',
    name: 'Laptop CLI',
    display: 'lsk_z9y8x7w6v5u4_…',
    uid: owner.uid,
    created_at: now - 400 * day,
    expires_at: now - 35 * day,
    last_used_at: null,
    scope: { type: 'full' },
  },
];

// The requests are answered by an axios adapter rather than MSW, so the
// stories also work where service workers are unavailable.
type Route = (config: InternalAxiosRequestConfig) => unknown;

const respond = (
  config: InternalAxiosRequestConfig,
  status: number,
  data: unknown
): Promise<AxiosResponse> => {
  const response: AxiosResponse = {
    data,
    status,
    statusText: status === 200 ? 'OK' : 'Forbidden',
    headers: {},
    config,
  };
  return status < 400
    ? Promise.resolve(response)
    : Promise.reject(
        new AxiosError(
          `Request failed with status code ${status}`,
          AxiosError.ERR_BAD_REQUEST,
          config,
          null,
          response
        )
      );
};

const SESSION_ONLY = {
  kind: 'PermissionDenied',
  causes: ['This action needs a signed-in session, not an API key'],
};

const routes = (
  user: PublicUser,
  list: ApiKeyInfo[] | 'session-only'
): Record<string, Route> => ({
  'get /user/info': () => user,
  'get /instance/list': () => instances,
  'get /global_settings': () => ({ safe_mode: true }),
  'get /user/api_keys': () => list,
  'post /user/api_keys': (config) => {
    const body = JSON.parse(config.data) as CreateApiKeyRequest;
    const created: CreatedApiKey = {
      key: 'lsk_n3wk3yst0ry1_NOT-A-REAL-KEY-only-a-storybook-example-value',
      info: {
        id: 'n3wk3yst0ry1',
        name: body.name,
        display: 'lsk_n3wk3yst0ry1_…',
        uid: user.uid,
        created_at: now,
        expires_at: body.expires_at,
        last_used_at: null,
        scope: body.scope,
      },
    };
    return created;
  },
});

const mockAdapter =
  (user: PublicUser, list: ApiKeyInfo[] | 'session-only') =>
  (config: InternalAxiosRequestConfig) => {
    const method = (config.method || 'get').toLowerCase();
    if (method === 'delete') return respond(config, 200, null);
    if (list === 'session-only' && config.url?.startsWith('/user/api_keys'))
      return respond(config, 403, SESSION_ONLY);
    const route = routes(user, list)[`${method} ${config.url}`];
    if (!route) return respond(config, 404, { kind: 'NotFound' });
    return respond(config, 200, route(config));
  };

// The default adapter, restored when a story using the mock is left
const defaultAdapter = axios.defaults.adapter;

const MockCore = ({
  user,
  list,
  children,
}: {
  user: PublicUser;
  list: ApiKeyInfo[] | 'session-only';
  children: ReactNode;
}) => {
  // layout effects run before the effects that send the queries' requests
  useLayoutEffect(() => {
    axios.defaults.adapter = mockAdapter(user, list);
    return () => {
      axios.defaults.adapter = defaultAdapter;
    };
  }, [user, list]);
  return children;
};

const withCore = (
  user: PublicUser,
  list: ApiKeyInfo[] | 'session-only'
): Decorator =>
  function WithCore(Story) {
    return (
      <MockCore user={user} list={list}>
        <QueryClientProvider client={new QueryClient()}>
          <LodestoneContext.Provider
            value={{
              core: {
                address: 'localhost',
                port: '16662',
                protocol: 'https',
                apiVersion: 'v1',
              },
              coreList: [],
              coreConnectionStatus: 'success',
              setCore: () => undefined,
              addCore: () => undefined,
              setCoreConnectionStatus: () => undefined,
              token: 'storybook',
              uid: user.uid,
              setToken: () => undefined,
              tokens: {},
            }}
          >
            <div className="flex w-160 flex-col">
              <Story />
            </div>
          </LodestoneContext.Provider>
        </QueryClientProvider>
      </MockCore>
    );
  };

const meta: Meta<typeof ApiKeysSection> = {
  title: 'settings/ApiKeysSection',
  component: ApiKeysSection,
  args: { enabled: true },
  globals: {
    backgrounds: { value: 'gray-850' },
  },
};
export default meta;

type Story = StoryObj<typeof ApiKeysSection>;

export const Owner: Story = {
  decorators: [withCore(owner, keys)],
};

export const MemberWithFewPermissions: Story = {
  decorators: [
    withCore(
      member,
      keys.map((k) => ({ ...k, uid: member.uid }))
    ),
  ],
};

export const NoKeys: Story = {
  decorators: [withCore(owner, [])],
};

export const ApiKeySession: Story = {
  decorators: [withCore(owner, 'session-only')],
};
