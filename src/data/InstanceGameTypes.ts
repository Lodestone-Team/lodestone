import { HandlerGameType } from "bindings/HandlerGameType";
import { useQuery } from '@tanstack/react-query';
import axios, { AxiosError } from 'axios';
import { ConfigurableManifest } from "components/Instance/Create/form";

export const InstanceGameTypes = () =>
  useQuery<HandlerGameType[], AxiosError>(
    ['games'],
    async () => {
      const response = await axios.get<HandlerGameType[]>('/games');
        return response.data;
    }
  );

export const SetupInstanceManifest = (game_type: HandlerGameType) => {
    return useQuery<ConfigurableManifest, AxiosError>(
        ['setup_manifest', game_type],
        async () => {
            const response = await axios.get<ConfigurableManifest>(`/setup_manifest/${game_type}`);
            return response.data;
        }
    );
}