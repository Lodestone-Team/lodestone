import { addInstance, deleteInstance, updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useContext, useEffect, useMemo, useRef } from 'react';
import { InstanceState } from 'bindings/InstanceState';
import { ClientEvent } from 'bindings/ClientEvent';
import { match, otherwise } from 'variant';
import { NotificationContext } from './NotificationContext';
import { EventQuery } from 'bindings/EventQuery';
import axios from 'axios';
import { LODESTONE_PORT } from 'utils/util';
import { UserPermission } from 'bindings/UserPermission';
import { PublicUser } from 'bindings/PublicUser';

/**
 * does not return anything, call this for the side effect of subscribing to the event stream
 * information will be available in the query cache of the respective query cache
 */
export const useEventStream = () => {
  const queryClient = useQueryClient();
  const { dispatch, ongoingDispatch } = useContext(NotificationContext);
  const { token, core, setCoreConnectionStatus } = useContext(LodestoneContext);
  const wsRef = useRef<WebSocket | null>(null);
  const wsConnected = useRef(false);

  const eventQuery: EventQuery = useMemo(
    () => ({
      bearer_token: token,
      event_levels: null,
      event_types: null,
      instance_event_types: null,
      user_event_types: null,
      event_instance_ids: null,
    }),
    [token]
  );

  const updateInstanceState = useCallback(
    (uuid: string, state: InstanceState) => {
      updateInstance(uuid, queryClient, (oldInfo) => {
        return { ...oldInfo, state };
      });
    },
    [queryClient]
  );
  const updateInstancePlayerCount = useCallback(
    (uuid: string, player_num: number) => {
      updateInstance(uuid, queryClient, (oldInfo) => {
        return {
          ...oldInfo,
          player_count: player_num,
        };
      });
    },
    [queryClient]
  );
  const updatePermission = useCallback(
    (permission: UserPermission) => {
      queryClient.setQueryData(
        ['user', 'info'],
        (oldInfo: PublicUser | undefined) => {
          if (!oldInfo) return oldInfo;
          return {
            ...oldInfo,
            permissions: permission,
          };
        }
      );
    },
    [queryClient]
  );

  const handleEvent = useCallback(
    (event: ClientEvent, fresh: boolean) => {
      const { event_inner, snowflake } = event;

      match(event_inner, {
        InstanceEvent: ({
          instance_event_inner: event_inner,
          instance_uuid: uuid,
          instance_name: name,
        }) =>
          match(event_inner, {
            StateTransition: ({ to }) => {
              if (fresh) updateInstanceState(uuid, to);
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
              });
            },
            InstanceWarning: () => {
              alert(
                "Warning: An instance has encountered a warning. This shouldn't happen, please report this to the developers."
              );
              dispatch({
                title: `Instance ${name} encountered a warning`,
                event,
                type: 'add',
              });
            },
            InstanceError: () => {
              if (fresh) updateInstanceState(uuid, 'Error');
              dispatch({
                title: `Instance ${name} encountered an error`,
                event,
                type: 'add',
              });
            },
            InstanceInput: ({ message }) => {
              console.log(`Got input on ${name}: ${message}`);
            },
            InstanceOutput: ({ message }) => {
              console.log(`Got output on ${name}: ${message}`);
            },
            SystemMessage: ({ message }) => {
              console.log(`Got system message on ${name}: ${message}`);
            },
            PlayerChange: ({ player_list, players_joined, players_left }) => {
              console.log(`Got player change on ${name}: ${player_list}`);
              console.log(`${players_joined} joined ${name}`);
              console.log(`${players_left} left ${name}`);
              if (fresh) updateInstancePlayerCount(uuid, player_list.length);

              // we don't need match statement on the type of player yet because there's only MinecraftPlayyer for now
              const player_list_names = player_list.map((p) => p.name);
              const players_joined_names = players_joined.map((p) => p.name);
              const players_left_names = players_left.map((p) => p.name);

              const title = `${
                players_joined.length > 0
                  ? `${players_joined_names.join(', ')} Joined ${name}`
                  : ''
              }
              ${
                players_left.length > 0 && players_joined.length > 0
                  ? ' and '
                  : ''
              }
              ${
                players_left.length > 0
                  ? `${players_left_names.join(', ')} Left ${name}`
                  : ''
              }`;
              dispatch({
                title,
                event,
                type: 'add',
              });
            },
            PlayerMessage: ({ player, player_message }) => {
              console.log(`${player} said ${player_message} on ${name}`);
              dispatch({
                title: `${player} said ${player_message} on ${name}`,
                event,
                type: 'add',
              });
            },
          }),
        UserEvent: ({ user_id: uid, user_event_inner: event_inner }) =>
          match(event_inner, {
            UserCreated: () => {
              console.log(`User ${uid} created`);
              dispatch({
                title: `User ${uid} created`,
                event,
                type: 'add',
              });
            },
            UserDeleted: () => {
              console.log(`User ${uid} deleted`);
              dispatch({
                title: `User ${uid} deleted`,
                event,
                type: 'add',
              });
            },
            UserLoggedIn: () => {
              console.log(`User ${uid} logged in`);
              dispatch({
                title: `User ${uid} logged in`,
                event,
                type: 'add',
              });
            },
            UserLoggedOut: () => {
              console.log(`User ${uid} logged out`);
              dispatch({
                title: `User ${uid} logged out`,
                event,
                type: 'add',
              });
            },
            PermissionChanged: (permission) => {
              console.log(`User ${uid} permission changed to ${permission}`);
              updatePermission(permission);
              dispatch({
                title: `User ${uid}'s permission has changed`,
                event,
                type: 'add',
              });
            },
          }),
        MacroEvent: ({
          instance_uuid: uuid,
          macro_pid,
          macro_event_inner: event_inner,
        }) =>
          match(event_inner, {
            MacroStarted: () => {
              console.log(`Macro ${macro_pid} started on ${uuid}`);
              dispatch({
                title: `Macro ${macro_pid} started on ${uuid}`,
                event,
                type: 'add',
              });
            },
            MacroStopped: () => {
              console.log(`Macro ${macro_pid} stopped on ${uuid}`);
              dispatch({
                title: `Macro ${macro_pid} stopped on ${uuid}`,
                event,
                type: 'add',
              });
            },
            MacroErrored: ({ error_msg }) => {
              console.log(`Macro ${macro_pid} errored on ${uuid}: ${error_msg}`);
              dispatch({
                title: `Macro ${macro_pid} errored on ${uuid}: ${error_msg}`,
                event,
                type: 'add',
              });
            },
          }),
        ProgressionEvent: (progressionEvent) => {
          ongoingDispatch({
            event,
            progressionEvent,
            dispatch,
          });
          // check if there's a "value"
          const inner = progressionEvent.progression_event_inner;
          if (fresh) {
            match(
              inner,
              otherwise(
                {
                  ProgressionEnd: ({ inner, message }) => {
                    if (!inner) return;
                    match(
                      inner,
                      otherwise(
                        {
                          InstanceCreation: (instance_info) =>
                            addInstance(instance_info, queryClient),
                          InstanceDelete: ({ instance_uuid: uuid }) =>
                            deleteInstance(uuid, queryClient),
                        },
                        // eslint-disable-next-line @typescript-eslint/no-empty-function
                        (_) => {}
                      )
                    );
                  },
                  ProgressionStart: ({ inner }) => {
                    if (!inner) return;
                    match(
                      inner,
                      otherwise(
                        {
                          // InstanceDelete: ({ instance_uuid: uuid }) =>
                          //   deleteInstance(uuid, queryClient),
                        },
                        // eslint-disable-next-line @typescript-eslint/no-empty-function
                        (_) => {}
                      )
                    );
                  },
                },
                // eslint-disable-next-line @typescript-eslint/no-empty-function
                (_) => {}
              )
            );
          }
        },
        FSEvent: ({ operation, target }) => {
          // console.log(`FS ${operation} on ${target.path}`);
          // match(target, {
          //   File: ({ path }) => {
          //     dispatch({
          //       title: `FS ${operation} on ${path}`,
          //       event,
          //       type: 'add',
          //     });
          //   },
          //   Directory: ({ path }) => {
          //     dispatch({
          //       title: `FS ${operation} on ${path}`,
          //       event,
          //       type: 'add',
          //     });
          //   },
          // });
        },
      });
    },
    [
      dispatch,
      ongoingDispatch,
      queryClient,
      updateInstancePlayerCount,
      updateInstanceState,
    ]
  );

  useEffect(() => {
    if (!token) return;

    dispatch({
      type: 'clear',
    });

    const bufferAddress = `/events/all/buffer?filter=${JSON.stringify(
      eventQuery
    )}`;

    axios.get<Array<ClientEvent>>(bufferAddress).then((response) => {
      response.data.forEach((event) => {
        handleEvent(event, false);
      });
    });
  }, [eventQuery, handleEvent, queryClient, token, core]);

  useEffect(() => {
    if (!token) return;

    const connectWebsocket = () => {
      const wsAddress = `ws://${core.address}:${
        core.port ?? LODESTONE_PORT
      }/api/${core.apiVersion}/events/all/stream?filter=${JSON.stringify(
        eventQuery
      )}`;

      if (wsRef.current) wsRef.current.close();

      const websocket = new WebSocket(wsAddress);

      wsRef.current = websocket;
      wsConnected.current = true;

      websocket.onopen = () => {
        console.log('websocket opened');
        setCoreConnectionStatus('success');
      };

      websocket.onerror = (event) => {
        console.error('websocket error', event);
        setCoreConnectionStatus('error');
        websocket.close();
      };

      websocket.onmessage = (messageEvent) => {
        const event: ClientEvent = JSON.parse(messageEvent.data);
        handleEvent(event, true);
      };

      websocket.onclose = () => {
        console.log('websocket closed');
        wsRef.current = null;
        if (!wsConnected.current) return;
        setTimeout(() => {
          console.log('reconnecting');
          connectWebsocket();
        }, 1000);
      };
    };

    connectWebsocket();
    return () => {
      console.log('unmounting event listener');
      wsConnected.current = false;
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, [handleEvent, token, eventQuery, handleEvent, core]);
};
