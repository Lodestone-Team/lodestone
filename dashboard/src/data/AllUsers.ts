import { useQuery } from '@tanstack/react-query';
import axios from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import { t } from 'i18n';

export const useAllUsers = (enabled = true) =>
  useQuery<{ [uid: string]: PublicUser }, Error>({
    queryKey: ['user', 'list'],
    queryFn: () =>
      axios.get<PublicUser[]>('/user/list').then((response) => {
        if (response.status !== 200) {
          throw new Error(t('shell.error.invalidStatusCode'));
        }
        if (!response.data) {
          throw new Error(t('shell.error.invalidResponse'));
        }
        return response.data.reduce(
          (acc, instance) => ({
            ...acc,
            [instance.uid]: instance,
          }),
          {}
        );
      }),
    enabled: enabled,
  });
