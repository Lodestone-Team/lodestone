import { addInstance, updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useContext, useEffect, useMemo } from 'react';
import { InstanceState } from 'bindings/InstanceState';
import { ClientEvent } from 'bindings/ClientEvent';
import { match, otherwise, partial } from 'variant';
import { NotificationContext } from './NotificationContext';
import { formatBytes, formatBytesDownload } from 'utils/util';
import { EventQuery } from 'bindings/EventQuery';
import axios from 'axios';

/**
 * does not return anything, call this for the side effect of subscribing to the event stream
 * information will be available in the query cache of the respective query cache
 */
export const useEventStream = () => {
  const queryClient = useQueryClient();
  const { dispatch, ongoingDispatch } = useContext(NotificationContext);
  const { isReady, token, address, port, apiVersion } =
    useContext(LodestoneContext);

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
    (uuid: string, increment: number) => {
      updateInstance(uuid, queryClient, (oldInfo) => {
        return {
          ...oldInfo,
          player_count: oldInfo.player_count
            ? oldInfo.player_count + increment
            : oldInfo.player_count,
        };
      });
    },
    [queryClient]
  );

  const handleEvent = useCallback(
    (event: ClientEvent) => {
      const { event_inner, snowflake_str } = event;

      match(event_inner, {
        InstanceEvent: ({
          instance_event_inner: event_inner,
          instance_uuid: uuid,
          instance_name: name,
        }) =>
          match(event_inner, {
            InstanceStarting: () => {
              updateInstanceState(uuid, 'Starting');
              dispatch({
                message: `Starting instance ${name}`,
                status: 'info',
                event,
              });
            },
            InstanceStarted: () => {
              updateInstanceState(uuid, 'Running');
              dispatch({
                message: `Instance ${name} started`,
                status: 'info',
                event,
              });
            },
            InstanceStopping: () => {
              updateInstanceState(uuid, 'Stopping');
              dispatch({
                message: `Stopping instance ${name}`,
                status: 'info',
                event,
              });
            },
            InstanceStopped: () => {
              updateInstanceState(uuid, 'Stopped');
              dispatch({
                message: `Instance ${name} stopped`,
                status: 'info',
                event,
              });
            },
            InstanceWarning: () => {
              alert(
                "Warning: An instance has encountered a warning. This shouldn't happen, please report this to the developers."
              );
              dispatch({
                message: `Instance ${name} encountered a warning`,
                status: 'error',
                event,
              });
            },
            InstanceError: () => {
              updateInstanceState(uuid, 'Error');
              dispatch({
                message: `Instance ${name} encountered an error`,
                status: 'error',
                event,
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
              // updateInstancePlayerCount(uuid, player_list.length);
              console.log(`Got player change on ${name}: ${player_list}`);
              console.log(`${players_joined} joined ${name}`);
              console.log(`${players_left} left ${name}`);
              updateInstancePlayerCount(uuid, players_joined.length);
              updateInstancePlayerCount(uuid, -players_left.length);
              const message = `${
                players_joined.length > 0
                  ? `${players_joined.join(', ')} Joined ${name}`
                  : ''
              }
            ${
              players_left.length > 0 && players_joined.length > 0
                ? ' and '
                : ''
            }
            ${
              players_left.length > 0
                ? `${players_left.join(', ')} Left ${name}`
                : ''
            }`;
              dispatch({
                message,
                status: 'info',
                event,
              });
            },
            PlayerMessage: ({ player, player_message }) => {
              console.log(`${player} said ${player_message} on ${name}`);
              dispatch({
                message: `${player} said ${player_message} on ${name}`,
                status: 'info',
                event,
              });
            },
          }),
        UserEvent: ({ user_id: uid, user_event_inner: event_inner }) =>
          match(event_inner, {
            UserCreated: () => {
              console.log(`User ${uid} created`);
              dispatch({
                message: `User ${uid} created`,
                status: 'info',
                event,
              });
            },
            UserDeleted: () => {
              console.log(`User ${uid} deleted`);
              dispatch({
                message: `User ${uid} deleted`,
                status: 'info',
                event,
              });
            },
            UserLoggedIn: () => {
              console.log(`User ${uid} logged in`);
              dispatch({
                message: `User ${uid} logged in`,
                status: 'info',
                event,
              });
            },
            UserLoggedOut: () => {
              console.log(`User ${uid} logged out`);
              dispatch({
                message: `User ${uid} logged out`,
                status: 'info',
                event,
              });
            },
          }),
        MacroEvent: ({
          instance_uuid: uuid,
          macro_uuid: macro_id,
          macro_event_inner: event_inner,
        }) =>
          match(event_inner, {
            MacroStarted: () => {
              console.log(`Macro ${macro_id} started on ${uuid}`);
              dispatch({
                message: `Macro ${macro_id} started on ${uuid}`,
                status: 'info',
                event,
              });
            },
            MacroStopped: () => {
              console.log(`Macro ${macro_id} stopped on ${uuid}`);
              dispatch({
                message: `Macro ${macro_id} stopped on ${uuid}`,
                status: 'info',
                event,
              });
            },
            MacroErrored: ({ error_msg }) => {
              console.log(`Macro ${macro_id} errored on ${uuid}: ${error_msg}`);
              dispatch({
                message: `Macro ${macro_id} errored on ${uuid}: ${error_msg}`,
                status: 'error',
                event,
              });
            },
          }),
        ProgressionEvent: (progressionEvent) => {
          ongoingDispatch({
            event,
            progressionEvent,
          });
          // check if there's a "value"
          const inner = progressionEvent.progression_event_inner;

          match(
            inner,
            otherwise(
              {
                ProgressionEnd: ({ value }) => {
                  if (!value) return;
                  match(value, {
                    InstanceInfo: (instance_info) =>
                      addInstance(instance_info, queryClient),
                  });
                },
              },
              // eslint-disable-next-line @typescript-eslint/no-empty-function
              (_) => {}
            )
          );
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
    if (!isReady) return;
    if (!token) return;

    const wsAddress = `ws://${address}:${
      port ?? 16662
    }/api/${apiVersion}/events/all/stream?filter=${JSON.stringify(eventQuery)}`;

    console.log(`connecting to ${wsAddress}`);

    const websocket = new WebSocket(wsAddress);

    // if the websocket because error, we should try to reconnect
    websocket.onerror = (event) => {
      console.error('websocket error', event);
      // alert('Disconnected from server, please refresh the page to reconnect');
    };

    websocket.onmessage = (messageEvent) => {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const event: ClientEvent = JSON.parse(messageEvent.data);
      handleEvent(event);
    };

    return () => {
      websocket.close();
    };
  }, [
    address,
    apiVersion,
    eventQuery,
    handleEvent,
    isReady,
    port,
    queryClient,
    token,
  ]);

  useEffect(() => {
    if (!isReady) return;
    if (!token) return;

    const bufferAddress = `/events/all/buffer?filter=${JSON.stringify(
      eventQuery
    )}`;

    axios.get<Array<ClientEvent>>(bufferAddress).then((response) => {
      response.data.forEach((event) => {
        handleEvent(event);
      });
    });
  }, [address, apiVersion, eventQuery, handleEvent, isReady, port, token]);
};
