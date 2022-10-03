import { InstanceState, updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useQueryClient } from '@tanstack/react-query';
import { useContext, useEffect } from 'react';

export interface DownloadProgress {
  total: number;
  downloaded: number;
  download_name: string;
}

export interface SetupProgress {
  current_step: [number, string];
  total_steps: number;
}

export type EventInner =
  | 'InstanceStarting'
  | 'InstanceStarted'
  | 'InstanceStopping'
  | 'InstanceStopped'
  | 'InstanceWarning'
  | 'InstanceError'
  | { InstanceInput: string }
  | { InstanceOutput: string }
  | { SystemMessage: string }
  | { PlayerChange: Array<string> }
  | { PlayerJoined: string }
  | { PlayerLeft: string }
  | { PlayerMessage: [string, string] }
  | { Downloading: DownloadProgress }
  | { Setup: SetupProgress };

export interface Event {
  event_inner: EventInner;
  instance_uuid: string;
  instance_name: string;
  details: string;
  timestamp: number;
  idempotency: string;
}

/**
 * does not return anything, call this for the side effect of subscribing to the event stream
 * information will be available in the query cache of the respective query cache
 */
export const useEventStream = () => {
  const queryClient = useQueryClient();
  const { address, port, apiVersion, isReady, token } = useContext(LodestoneContext);

  useEffect(() => {
    const updateInstanceState = (uuid: string, state: InstanceState) => {
      updateInstance(uuid, queryClient, (oldInfo) => {
        return { ...oldInfo, state };
      });
    };
    const updateInstancePlayerCount = (uuid: string, increment: number) => {
      updateInstance(uuid, queryClient, (oldInfo) => {
        return { ...oldInfo, player_count: oldInfo.player_count + increment };
      });
    };

    if (!isReady) return;
    if (!token) return;

    const websocket = new WebSocket(
      `ws://${address}:${
        port ?? 3000
      }/api/${apiVersion}/events/all/stream?token=Bearer ${token}`
    );
    websocket.onmessage = (messageEvent) => {
      const {
        event_inner: details,
        instance_uuid: uuid,
        instance_name: name,
      }: Event = JSON.parse(messageEvent.data);
      // do something different for each event type
      switch (details) {
        case 'InstanceStarting':
          updateInstanceState(uuid, 'Starting');
          break;
        case 'InstanceStarted':
          updateInstanceState(uuid, 'Running');
          break;
        case 'InstanceStopping':
          updateInstanceState(uuid, 'Stopping');
          break;
        case 'InstanceStopped':
          updateInstanceState(uuid, 'Stopped');
          break;
        case 'InstanceWarning':
          alert(
            `¯\\_(ツ)_/¯ Got a warning on ${name}: ${details}, who knows what that means`
          );
          break;
        case 'InstanceError':
          updateInstanceState(uuid, 'Error');
          break;
      }

      // now handle the object types
      if (typeof details === 'object') {
        if ('InstanceInput' in details) {
          console.log(`Got input on ${name}: ${details.InstanceInput}`);
        } else if ('InstanceOutput' in details) {
          console.log(`Got output on ${name}: ${details.InstanceOutput}`);
        } else if ('SystemMessage' in details) {
          console.log(
            `Got system message on ${name}: ${details.SystemMessage}`
          );
        } else if ('PlayerChange' in details) {
          console.log(
            `Players on ${name} are now : ${details.PlayerChange.join(', ')}`
          );
        } else if ('PlayerJoined' in details) {
          console.log(`Player joined ${name}: ${details.PlayerJoined}`);
          updateInstancePlayerCount(uuid, 1);
        } else if ('PlayerLeft' in details) {
          console.log(`Player left ${name}: ${details.PlayerLeft}`);
          updateInstancePlayerCount(uuid, -1);
        } else if ('PlayerMessage' in details) {
          console.log(
            `Player ${details.PlayerMessage[0]} said ${details.PlayerMessage[1]}`
          );
        } else if ('Downloading' in details) {
          console.log(
            `Downloading ${details.Downloading.download_name} ${details.Downloading.downloaded}/${details.Downloading.total}`
          );
        } else if ('Setup' in details) {
          console.log(
            `Setting up ${name} ${details.Setup.current_step[0]}/${details.Setup.total_steps}`
          );
        }
      }
    };

    return () => {
      websocket.close();
    };
  }, [queryClient, address, port, apiVersion, isReady, token]);
};
