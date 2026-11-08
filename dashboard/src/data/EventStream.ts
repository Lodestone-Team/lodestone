import { useUid } from 'data/UserInfo';
import { addInstance, deleteInstance, updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';

import { useContext, useEffect, useLayoutEffect, useRef } from 'react';
import { InstanceState } from 'bindings/InstanceState';
import { ClientEvent } from 'bindings/ClientEvent';
import { match, otherwise } from 'variant';
import { NotificationContext } from './NotificationContext';
import { EventQuery } from 'bindings/EventQuery';
import axios from 'axios';
import { coreWebSocketUrl, errorToString, isAxiosError } from 'utils/util';
import { UserPermission } from 'bindings/UserPermission';
import { PublicUser } from 'bindings/PublicUser';
import { toast } from 'react-toastify';
import { Player } from 'bindings/Player';
import { QueryClient, useQueryClient } from '@tanstack/react-query';
import { openReconnectingWebSocket } from 'utils/websocket';
import { apiKeyKeys } from 'data/ApiKeys';

/**
 * The query for both the event buffer and the event stream.
 * The token is only needed for the websocket, since browsers cannot set
 * headers on websockets. REST requests send it in the Authorization header.
 */
const eventQuery = (bearer_token: string | null): EventQuery => ({
  bearer_token,
  event_levels: null,
  event_types: null,
  instance_event_types: null,
  user_event_types: null,
  event_user_ids: null,
  event_instance_ids: null,
  time_range: null,
});

/**
 * Console lines are shown by the console's own websocket (ConsoleStream),
 * the global event stream has nothing to do with them.
 */
const isConsoleEvent = (event: ClientEvent) =>
  event.event_inner.type === 'InstanceEvent' &&
  (event.event_inner.instance_event_inner.type === 'InstanceOutput' ||
    event.event_inner.instance_event_inner.type === 'InstanceInput' ||
    event.event_inner.instance_event_inner.type === 'SystemMessage');

const updateInstanceState = (
  queryClient: QueryClient,
  uuid: string,
  state: InstanceState
) => updateInstance(uuid, queryClient, (oldInfo) => ({ ...oldInfo, state }));

const updateInstancePlayerList = (
  queryClient: QueryClient,
  uuid: string,
  players: Player[]
) =>
  updateInstance(uuid, queryClient, (oldInfo) => ({
    ...oldInfo,
    player_list: players,
    player_count: players.length,
  }));

const updateUserInList = (
  queryClient: QueryClient,
  uid: string,
  update: (user: PublicUser) => PublicUser
) =>
  queryClient.setQueryData(
    ['user', 'list'],
    (oldList: { [uid: string]: PublicUser } | undefined) => {
      if (!oldList || !oldList[uid]) return oldList;
      return { ...oldList, [uid]: update(oldList[uid]) };
    }
  );

const updateSelf = (
  queryClient: QueryClient,
  update: (user: PublicUser) => PublicUser
) =>
  queryClient.setQueryData(
    ['user', 'info'],
    (oldInfo: PublicUser | undefined) => (oldInfo ? update(oldInfo) : oldInfo)
  );

const setPermission = (permissions: UserPermission) => (user: PublicUser) => ({
  ...user,
  permissions,
});

/**
 * does not return anything, call this for the side effect of subscribing to the event stream
 * information will be available in the query cache of the respective query cache
 */
export const useEventStream = () => {
  const queryClient = useQueryClient();
  const { dispatch, ongoingDispatch } = useContext(NotificationContext);
  const selfUid = useUid();
  const { token, core, setCoreConnectionStatus, setToken } =
    useContext(LodestoneContext);
  const socket = `${core.address}:${core.port}`;

  const handleEvent = (event: ClientEvent, fresh: boolean) => {
    const { event_inner } = event;

    match(event_inner, {
      InstanceEvent: ({
        instance_event_inner: event_inner,
        instance_uuid: uuid,
        instance_name: name,
      }) =>
        match(event_inner, {
          StateTransition: ({ to }) => {
            if (fresh) updateInstanceState(queryClient, uuid, to);
            dispatch({
              title: `Instance ${name} ${
                {
                  Starting: `is starting`,
                  Running: `started`,
                  Stopping: `is stopping`,
                  Stopped: `stopped`,
                  Error: `encountered an error`,
                }[to]
              }!`,
              event,
              type: 'add',
              fresh,
            });
          },
          InstanceWarning: () => {
            toast.error(
              "Warning: An instance has encountered a warning. This shouldn't happen, please report this to the developers."
            );
            dispatch({
              title: `Instance ${name} encountered a warning`,
              event,
              type: 'add',
              fresh,
            });
          },
          InstanceError: () => {
            if (fresh) updateInstanceState(queryClient, uuid, 'Error');
            dispatch({
              title: `Instance ${name} encountered an error`,
              event,
              type: 'add',
              fresh,
            });
          },
          // console lines are filtered out before they get here
          InstanceInput: () => {},
          InstanceOutput: () => {},
          SystemMessage: () => {},
          PlayerChange: ({ player_list, players_joined, players_left }) => {
            if (fresh) updateInstancePlayerList(queryClient, uuid, player_list);

            // we don't need match statement on the type of player yet because there's only MinecraftPlayyer for now
            const players_joined_names = players_joined.map((p) => p.name);
            const players_left_names = players_left.map((p) => p.name);

            const title =
              (players_joined.length > 0
                ? `${players_joined_names.join(', ')} Joined ${name}`
                : '') +
              (players_left.length > 0 && players_joined.length > 0
                ? ' and '
                : '') +
              (players_left.length > 0
                ? `${players_left_names.join(', ')} Left ${name}`
                : '');
            if (title.length > 0)
              dispatch({
                title,
                event,
                type: 'add',
                fresh,
              });
          },
          PlayerMessage: ({ player, player_message }) => {
            dispatch({
              title: `${player} said ${player_message} on ${name}`,
              event,
              type: 'add',
              fresh,
            });
          },
        }),
      UserEvent: ({ user_id: uid, user_event_inner: event_inner }) => {
        if (!fresh) return;
        match(event_inner, {
          UserCreated: () => {
            queryClient.invalidateQueries({ queryKey: ['user', 'list'] });
          },
          UserDeleted: () => {
            if (uid === selfUid) {
              // the user deleted themselves, log out
              setToken('', socket);
              queryClient.setQueryData(['user', 'info'], undefined);
            }
            queryClient.setQueryData(
              ['user', 'list'],
              (oldList: { [uid: string]: PublicUser } | undefined) => {
                if (!oldList) return oldList;
                const newList = { ...oldList };
                delete newList[uid];
                return newList;
              }
            );
          },
          UserLoggedIn: () => {},
          UserLoggedOut: () => {},
          UsernameChanged: ({ new_username }) => {
            const rename = (user: PublicUser) => ({
              ...user,
              username: new_username,
            });
            if (uid === selfUid) updateSelf(queryClient, rename);
            updateUserInList(queryClient, uid, rename);
          },
          PermissionChanged: ({ new_permissions }) => {
            if (uid === selfUid)
              updateSelf(queryClient, setPermission(new_permissions));
            updateUserInList(queryClient, uid, setPermission(new_permissions));
          },
          ApiKeyCreated: ({ name: keyName }) => {
            refreshApiKeys(queryClient, uid, uid === selfUid);
            dispatch({
              title: `API key "${keyName}" created`,
              event,
              type: 'add',
              fresh,
            });
          },
          ApiKeyRevoked: ({ name: keyName }) => {
            refreshApiKeys(queryClient, uid, uid === selfUid);
            dispatch({
              title: `API key "${keyName}" revoked`,
              event,
              type: 'add',
              fresh,
            });
          },
        });
      },
      MacroEvent: ({
        instance_uuid: uuid,
        macro_pid,
        macro_event_inner: event_inner,
      }) =>
        match(event_inner, {
          Started: () => {
            if (fresh) {
              queryClient.invalidateQueries({
                queryKey: ['instance', uuid, 'taskList'],
              });
              queryClient.invalidateQueries({
                queryKey: ['instance', uuid, 'macroList'],
              });
            }
            dispatch({
              title: `Macro ${macro_pid} started on ${uuid}`,
              event,
              type: 'add',
              fresh,
            });
          },
          Detach: () => {
            dispatch({
              title: `Macro ${macro_pid} detached on ${uuid}`,
              event,
              type: 'add',
              fresh,
            });
          },
          Stopped: ({ exit_status }) => {
            if (fresh) {
              queryClient.invalidateQueries({
                queryKey: ['instance', uuid, 'taskList'],
              });
              queryClient.invalidateQueries({
                queryKey: ['instance', uuid, 'historyList'],
              });
            }
            dispatch({
              title: `Macro ${macro_pid} stopped on ${uuid} with status ${exit_status.type}`,
              event,
              type: 'add',
              fresh,
            });
          },
        }),
      PlayitggRunnerEvent: ({ playitgg_runner_event_inner: event_inner }) => {
        match(event_inner, {
          RunnerStarted: () => {
            if (fresh)
              queryClient.setQueryData(['playitgg', 'status'], 'Started');
            dispatch({
              title: 'Playitgg runner started!',
              event,
              type: 'add',
              fresh,
            });
          },
          RunnerLoading: () => {
            if (fresh)
              queryClient.setQueryData(['playitgg', 'status'], 'Loading');
            dispatch({
              title: 'Playitgg runner loading...',
              event,
              type: 'add',
              fresh,
            });
          },
          RunnerStopped: () => {
            if (fresh)
              queryClient.setQueryData(['playitgg', 'status'], 'Stopped');
            dispatch({
              title: 'Playitgg runner stopped!',
              event,
              type: 'add',
              fresh,
            });
          },
        });
      },
      ProgressionEvent: (progressionEvent) => {
        ongoingDispatch({
          event,
          progressionEvent,
          fresh,
        });
        if (!fresh) return;
        match(
          progressionEvent.progression_event_inner,
          otherwise(
            {
              ProgressionEnd: ({ inner }) => {
                if (!inner) return;
                match(
                  inner,
                  otherwise(
                    {
                      InstanceCreation: (instance_info) =>
                        addInstance(instance_info, queryClient),
                      InstanceDelete: ({ instance_uuid: uuid }) =>
                        deleteInstance(uuid, queryClient),
                      FSOperationCompleted: ({
                        instance_uuid,
                        success,
                        message,
                      }) => {
                        if (success) {
                          toast.success(message);
                        } else {
                          toast.error(message);
                        }
                        queryClient.invalidateQueries({
                          queryKey: ['instance', instance_uuid, 'fileList'],
                        });
                      },
                    },
                    () => {}
                  )
                );
              },
            },
            () => {}
          )
        );
      },
      FSEvent: () => {},
      // e.g. the code execution switch changed; the event has no payload
      GlobalSettingsChanged: () => {
        if (fresh)
          queryClient.invalidateQueries({ queryKey: ['global_settings'] });
      },
    });
  };

  // The websocket below lives as long as token and core don't change, but the
  // handler must always see the latest selfUid, setToken and contexts.
  const handleEventRef = useRef(handleEvent);
  useLayoutEffect(() => {
    handleEventRef.current = handleEvent;
  });

  // fetch the events that happened before we connected
  useEffect(() => {
    if (!token) return;
    let cancelled = false;

    dispatch({
      type: 'clear',
    });

    axios
      .get<Array<ClientEvent>>('/events/all/buffer', {
        params: { filter: JSON.stringify(eventQuery(null)) },
      })
      .then((response) => {
        if (cancelled) return;
        response.data.forEach((event) => {
          if (!isConsoleEvent(event)) handleEventRef.current(event, false);
        });
      })
      .catch((error) => {
        if (cancelled) return;
        // an expired token is handled by the 401 interceptor (pages/index.tsx)
        if (isAxiosError(error) && error.response?.status === 401) return;
        console.error('Failed to fetch the event buffer', error);
        toast.error(`Failed to load notifications: ${errorToString(error)}`, {
          toastId: 'eventBufferError',
        });
      });
    return () => {
      cancelled = true;
    };
  }, [dispatch, token, core]);

  // subscribe to new events
  useEffect(() => {
    if (!token) return;

    const websocket = openReconnectingWebSocket({
      url: () =>
        coreWebSocketUrl(
          core,
          `/events/all/stream?filter=${encodeURIComponent(
            JSON.stringify(eventQuery(token))
          )}`
        ),
      onOpen: (_, isReconnect) => {
        setCoreConnectionStatus('success');
        // events sent while we were disconnected are lost, refetch what they would have updated
        if (isReconnect) {
          queryClient.invalidateQueries({ queryKey: ['instances', 'list'] });
          queryClient.invalidateQueries({ queryKey: ['user', 'info'] });
          queryClient.invalidateQueries({ queryKey: ['global_settings'] });
        }
      },
      onMessage: (messageEvent) => {
        let event: ClientEvent;
        try {
          event = JSON.parse(messageEvent.data);
        } catch (e) {
          console.error('Invalid event from the event stream', e);
          return;
        }
        if (!isConsoleEvent(event)) handleEventRef.current(event, true);
      },
      onClose: async (_, opened) => {
        // a dropped connection is retried right away; the status only changes
        // if the retry fails
        if (opened) return true;
        // Browsers don't expose why a websocket handshake failed, so ask the
        // REST API whether the token is still valid and the core is reachable.
        try {
          await axios.get('/user/info');
          // the core is up and the token is valid, only the websocket fails
          setCoreConnectionStatus('degraded');
          return true;
        } catch (error) {
          if (isAxiosError(error) && error.response?.status === 401) {
            // the 401 interceptor logs the user out, which closes this socket
            return false;
          }
          setCoreConnectionStatus(
            isAxiosError(error) && !error.response ? 'error' : 'degraded'
          );
          return true;
        }
      },
    });

    return () => {
      websocket.close();
    };
  }, [token, core, queryClient, setCoreConnectionStatus]);
};

/** Refetches the API key lists a key event may have changed. */
const refreshApiKeys = (
  queryClient: QueryClient,
  uid: string,
  isSelf: boolean
) => {
  if (isSelf) queryClient.invalidateQueries({ queryKey: apiKeyKeys.own });
  queryClient.invalidateQueries({ queryKey: apiKeyKeys.ofUser(uid) });
};
