import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { PublicUser } from 'bindings/PublicUser';

export const useAllUsers = (enabled = true) =>
  useQuery<{ [uid: string]: PublicUser }, Error>(
    ['user', 'list'],
    () =>
      axios.get<PublicUser[]>('/user/list').then((response) => {
        if (response.status !== 200) {
          throw new Error('Invalid status code');
        }
        if (!response.data) {
          throw new Error('Invalid response');
        }
        return response.data.reduce(
          (acc, instance) => ({
            ...acc,
            [instance.uid]: instance,
          }),
          {}
        );
      }),
    {
      enabled: enabled,
    }
  );
