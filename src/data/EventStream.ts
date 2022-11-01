import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useQueryClient } from '@tanstack/react-query';
import { useContext, useEffect } from 'react';
import { InstanceState } from 'bindings/InstanceState';
import { ClientEvent } from 'bindings/ClientEvent';
import { match } from 'variant';

// type to represent a notification on the frontent

export type NotificationType = 'error' | 'info' | 'success';

export type DashboardNotification = {
  type: NotificationType;
  message: string;
  timestamp: bigint;
  key: string;
};

type NotificationUnique = {
  type: NotificationType;
  message: string;
};

const error = (message: string) =>
  ({
    type: 'error',
    message,
  } as NotificationUnique);

const info = (message: string) =>
  ({
    type: 'info',
    message,
  } as NotificationUnique);

const success = (message: string) =>
  ({
    type: 'success',
    message,
  } as NotificationUnique);

/**
 * does not return anything, call this for the side effect of subscribing to the event stream
 * information will be available in the query cache of the respective query cache
 */
export const useEventStream = () => {
  const queryClient = useQueryClient();
  const { address, port, apiVersion, isReady, token, pushNotification } =
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
        port ?? 3000
      }/api/${apiVersion}/events/all/stream?token=Bearer ${token}`
    );

    // if the websocket because error, we should try to reconnect
    websocket.onerror = (event) => {
      console.error('websocket error', event);
      alert('Disconnected from server, please refresh the page to reconnect');
    };

    websocket.onmessage = (messageEvent) => {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const { event_inner, timestamp, idempotency }: ClientEvent = JSON.parse(
        messageEvent.data
      );

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
              return info(`Instance ${name} is starting`);
            },
            InstanceStarted: () => {
              updateInstanceState(uuid, 'Running');
              return success(`Instance ${name} is running`);
            },
            InstanceStopping: () => {
              updateInstanceState(uuid, 'Stopping');
              return info(`Instance ${name} is stopping`);
            },
            InstanceStopped: () => {
              updateInstanceState(uuid, 'Stopped');
              return success(`Instance ${name} is stopped`);
            },
            InstanceWarning: () => {
              alert(
                "Warning: An instance has encountered a warning. This shouldn't happen, please report this to the developers."
              );
              return error('Instance ${name} has encountered a warning');
            },
            InstanceError: () => {
              updateInstanceState(uuid, 'Error');
              return error(`Instance ${name} has encountered an error`);
            },
            InstanceCreationFailed: () => {
              alert(
                "Failed to create instance. This shouldn't happen, please report this to the developers."
              );
              return error(`Failed to create instance ${name}`);
            },
            InstanceInput: ({ message }) => {
              console.log(`Got input on ${name}: ${message}`);
              return null;
            },
            InstanceOutput: ({ message }) => {
              console.log(`Got output on ${name}: ${message}`);
              return null;
            },
            SystemMessage: ({ message }) => {
              console.log(`Got system message on ${name}: ${message}`);
              return null;
            },
            PlayerChange: ({ player_list, players_joined, players_left }) => {
              // updateInstancePlayerCount(uuid, player_list.length);
              console.log(`Got player change on ${name}: ${player_list}`);
              console.log(`${players_joined} joined ${name}`);
              console.log(`${players_left} left ${name}`);
              updateInstancePlayerCount(uuid, players_joined.length);
              updateInstancePlayerCount(uuid, -players_left.length);
              return info(
                `${players_joined.length} joined ${name}, ${players_left.length} left ${name}`
              );
            },
            PlayerJoined: ({ player }) => {
              console.log(`Deprecated PlayerJoined event on ${name}`);
              return null;
            },
            PlayerLeft: ({ player }) => {
              console.log(`Deprecated PlayerLeft event on ${name}`);
              return null;
            },
            PlayerMessage: ({ player, player_message }) => {
              console.log(`${player} said ${player_message} on ${name}`);
              return info(`${player} said ${player_message} on ${name}`);
            },
            Downloading: ({ total, downloaded, download_name }) => {
              console.log(
                `Downloading ${name}: ${downloaded}/${total} (${download_name})`
              );
              return info(
                `Downloading ${name}: ${downloaded}/${total} (${download_name})`
              );
            },
            Setup: ({ current_step, total_steps }) => {
              const [current, stepName] = current_step;
              console.log(
                `Setting up ${name}: ${current}/${total_steps} (${stepName})`
              );
              return info(
                `Setting up ${name}: ${current}/${total_steps} (${stepName})`
              );
            },
          }),
        UserEvent: ({ user_id: uid, user_event_inner: event_inner }) =>
          match(event_inner, {
            UserCreated: () => {
              console.log(`User ${uid} created`);
              return info(`User ${uid} created`);
            },
            UserDeleted: () => {
              console.log(`User ${uid} deleted`);
              return info(`User ${uid} deleted`);
            },
            UserLoggedIn: () => {
              console.log(`User ${uid} logged in`);
              return info(`User ${uid} logged in`);
            },
            UserLoggedOut: () => {
              console.log(`User ${uid} logged out`);
              return info(`User ${uid} logged out`);
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
              return info(`Macro ${macro_id} started on ${uuid}`);
            },
            MacroStopped: () => {
              console.log(`Macro ${macro_id} stopped on ${uuid}`);
              return info(`Macro ${macro_id} stopped on ${uuid}`);
            },
            MacroErrored: ({ error_msg }) => {
              console.log(`Macro ${macro_id} errored on ${uuid}: ${error_msg}`);
              return info(`Macro ${macro_id} errored on ${uuid}: ${error_msg}`);
            },
          }),
      });

      const notification: DashboardNotification | null = notificationUnique
        ? {
            ...notificationUnique,
            timestamp,
            key: idempotency,
          }
        : null;

      if (notification) pushNotification(notification);
    };

    return () => {
      websocket.close();
    };
  }, [queryClient, address, port, apiVersion, isReady, token]);
};
