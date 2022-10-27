import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useQueryClient } from '@tanstack/react-query';
import { useContext, useEffect } from 'react';
import { InstanceState } from 'bindings/InstanceState';
import { ClientEvent } from 'bindings/ClientEvent';
import { match } from 'variant';

/**
 * does not return anything, call this for the side effect of subscribing to the event stream
 * information will be available in the query cache of the respective query cache
 */
export const useEventStream = () => {
  const queryClient = useQueryClient();
  const { address, port, apiVersion, isReady, token } =
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
      alert("Disconnected from server, please refresh the page to reconnect");
    };

    websocket.onmessage = (messageEvent) => {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const { event_inner, timestamp, idempotency }: ClientEvent = JSON.parse(
        messageEvent.data
      );

      // I love match statements
      match(event_inner, {
        InstanceEvent: ({
          instance_event_inner: event_inner,
          instance_uuid: uuid,
          instance_name: name,
        }) => {
          match(event_inner, {
            InstanceStarting: () => {
              updateInstanceState(uuid, 'Starting');
            },
            InstanceStarted: () => {
              updateInstanceState(uuid, 'Running');
            },
            InstanceStopping: () => {
              updateInstanceState(uuid, 'Stopping');
            },
            InstanceStopped: () => {
              updateInstanceState(uuid, 'Stopped');
            },
            InstanceWarning: () => {
              alert(
                "Warning: An instance has encountered a warning. This shouldn't happen, please report this to the developers."
              );
              // TODO: remove alert and replace with something more elegant
            },
            InstanceError: () => {
              updateInstanceState(uuid, 'Error');
            },
            InstanceCreationFailed: () => {
              alert(
                "Failed to create instance. This shouldn't happen, please report this to the developers."
              );
              // TODO: remove alert and instead show a notification
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
            PlayerChange: ({ player_list }) => {
              // updateInstancePlayerCount(uuid, player_list.length);
              console.log(`Got player change on ${name}: ${player_list}`);
              // TODO: update player list, use namemc api to get player icons too maybe?
            },
            PlayerJoined: ({ player }) => {
              console.log(`${player} joined ${name}`);
              updateInstancePlayerCount(uuid, 1);
            },
            PlayerLeft: ({ player }) => {
              console.log(`${player} left ${name}`);
              updateInstancePlayerCount(uuid, -1);
            },
            PlayerMessage: ({ player, player_message }) => {
              console.log(`${player} said ${player_message} on ${name}`);
            },
            Downloading: ({ total, downloaded, download_name }) => {
              console.log(
                `Downloading ${name}: ${downloaded}/${total} (${download_name})`
              );
            },
            Setup: ({ current_step, total_steps }) => {
              const [current, stepName] = current_step;
              console.log(
                `Setting up ${name}: ${current}/${total_steps} (${stepName})`
              );
            },
          });
        },
        UserEvent: ({ user_id: uid, user_event_inner: event_inner }) => {
          match(event_inner, {
            UserCreated: () => {
              console.log(`User ${uid} created`);
            },
            UserDeleted: () => {
              console.log(`User ${uid} deleted`);
            },
            UserLoggedIn: () => {
              console.log(`User ${uid} logged in`);
            },
            UserLoggedOut: () => {
              console.log(`User ${uid} logged out`);
            },
          });
        },
        MacroEvent: ({ instance_uuid: uuid, macro_uuid: macro_id, macro_event_inner: event_inner }) => {
          match(event_inner, {
            MacroStarted: () => {
              console.log(`Macro ${macro_id} started on ${uuid}`);
            },
            MacroStopped: () => {
              console.log(`Macro ${macro_id} stopped on ${uuid}`);
            },
            MacroErrored: ({error_msg}) => {
              console.log(`Macro ${macro_id} errored on ${uuid}: ${error_msg}`);
            },
          });
        },
      });
    };

    return () => {
      websocket.close();
    };
  }, [queryClient, address, port, apiVersion, isReady, token]);
};
