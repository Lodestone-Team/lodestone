import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { useContext } from 'react';
import { LodestoneContext } from './LodestoneContext';

export type InstanceState =
  | 'Starting'
  | 'Running'
  | 'Stopping'
  | 'Stopped'
  | 'Error';

export interface InstanceInfo {
  uuid: string;
  name: string;
  port: number;
  description: string;
  game_type: string;
  flavour: string;
  state: InstanceState;
  player_count: number;
  max_player_count: number;
  creation_time: number;
}

export const useInstanceList = () =>
  useQuery<{ [uuid: string]: InstanceInfo }, string>(
    ['instances', 'list'],
    () => {
      return axios
        .get<InstanceInfo[]>('/instances/list')
        .then((response) => {
          return response.data.reduce(
            (acc, instance) => ({
              ...acc,
              [instance.uuid]: instance,
            }),
            {}
          );
        });
    },
    {
      enabled: useContext(LodestoneContext).isReady,
    }
  );
