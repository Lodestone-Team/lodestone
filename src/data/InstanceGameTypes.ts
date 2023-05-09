import { HandlerGameType } from 'bindings/HandlerGameType';
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { ConfigurableManifest } from 'components/Instance/Create/form';

export const InstanceGameTypes = () =>
  useQuery<HandlerGameType[], AxiosError>(['games'], () =>
    axios.get<HandlerGameType[]>('/games').then((res) => res.data)
  );

export const SetupInstanceManifest = (game_type: HandlerGameType) =>
  useQuery<ConfigurableManifest, AxiosError>(
    ['setup_manifest', game_type],
    () =>
      axios
        .get<ConfigurableManifest>(`/setup_manifest/${game_type}`)
        .then((res) => res.data)
  );
