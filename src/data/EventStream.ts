import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useQueryClient } from '@tanstack/react-query';
import { useContext, useEffect } from 'react';
import { InstanceState } from 'bindings/InstanceState';
import { ClientEvent } from 'bindings/ClientEvent';
import { match } from 'variant';
import { NotificationContext } from './NotificationContext';
import { formatBytes, formatBytesDownload } from 'utils/util';

/**
 * does not return anything, call this for the side effect of subscribing to the event stream
 * information will be available in the query cache of the respective query cache
 */
export const useEventStream = () => {
  const queryClient = useQueryClient();
  const { dispatch, ongoingDispatch } = useContext(NotificationContext);
  const { isReady, token, address, port, apiVersion } =
    useContext(LodestoneContext);

  useEffect(() => {
    const updateInstanceState = (uuid: string, state: InstanceState) => {
      updateInstance(uuid, queryClient, (oldInfo) => {
        return { ...oldInfo, state };
      });
    };
    const updateInstancePlayerCount = (uuid: string, increment: number) => {
      updateInstance(uuid, queryClient, (oldInfo) => {
        return {
          ...oldInfo,
          player_count: oldInfo.player_count
            ? oldInfo.player_count + increment
            : oldInfo.player_count,
        };
      });
    };

    if (!isReady) return;
    if (!token) return;

    const websocket = new WebSocket(
      `ws://${address}:${
        port ?? 16662
      }/api/${apiVersion}/events/all/stream?token=Bearer ${token}`
    );

    // if the websocket because error, we should try to reconnect
    websocket.onerror = (event) => {
      console.error('websocket error', event);
      // alert('Disconnected from server, please refresh the page to reconnect');
    };

    websocket.onmessage = (messageEvent) => {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const event: ClientEvent = JSON.parse(messageEvent.data);
      const { event_inner, snowflake_str, idempotency } = event;

      // I love match statements
      const notificationUnique = match(event_inner, {
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
            InstanceCreationFailed: () => {
              alert(
                "Failed to create instance. This shouldn't happen, please report this to the developers."
              );
              ongoingDispatch({
                type: 'error',
                message: `Failed to create instance ${name}`,
                event,
                ongoing_key: uuid,
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
            Downloading: ({ total, downloaded, download_name }) => {
              console.log(
                `Downloading for ${name}: ${downloaded}/${total} (${download_name})`
              );
              if (!total) {
                const downloadStr = formatBytes(Number(downloaded));
                ongoingDispatch({
                  type: 'update',
                  message: `Downloading ${download_name}: ${downloadStr}`,
                  event,
                  ongoing_key: uuid,
                });
              } else {
                const downloadedStr = formatBytesDownload(
                  Number(downloaded),
                  Number(total)
                );
                const totalStr = formatBytes(Number(total));

                ongoingDispatch({
                  type: 'update',
                  message: `Downloading ${download_name}: ${downloadedStr} of ${totalStr}`,
                  progress: Number(downloaded) / Number(total) / 4 + 0.75,
                  // hard coded number manipulation because downloading is the 4/4th step of instance creation
                  event,
                  ongoing_key: uuid,
                });
              }
            },
            Setup: ({ current_step, total_steps }) => {
              const [current, stepName] = current_step;
              console.log(
                `Setting up ${name}: ${current}/${total_steps} (${stepName})`
              );
              const done = current - 1;
              if (done === 0)
                ongoingDispatch({
                  type: 'add',
                  title: `Setting up ${name}`,
                  message: `${current}/${total_steps} (${stepName})`,
                  progress: done / total_steps,
                  event,
                  ongoing_key: uuid,
                });
              else
                ongoingDispatch({
                  type: 'update',
                  message: `${name}: ${current}/${total_steps} (${stepName})`,
                  progress: done / total_steps,
                  event,
                  ongoing_key: uuid,
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
      });
    };

    return () => {
      websocket.close();
    };
  }, [address, apiVersion, isReady, port, queryClient, token]);
};
