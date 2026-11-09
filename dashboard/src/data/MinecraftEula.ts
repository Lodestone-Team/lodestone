import { EulaStatus } from 'bindings/EulaStatus';
import { InstanceUuid } from 'bindings/InstanceUuid';
import { axiosWrapper } from 'utils/util';

/** Where users read Mojang's Minecraft EULA. */
export const MINECRAFT_EULA_URL = 'https://aka.ms/MinecraftEULA';

/** Whether the user accepted the Minecraft EULA for a Minecraft server. */
export const getInstanceEula = (uuid: InstanceUuid) =>
  axiosWrapper<EulaStatus>({
    method: 'get',
    url: `/instance/${uuid}/eula`,
  });

/**
 * Accepts the Minecraft EULA for a Minecraft server. Only call this after the
 * user agreed to it.
 */
export const acceptInstanceEula = (uuid: InstanceUuid) =>
  axiosWrapper<EulaStatus>({
    method: 'put',
    url: `/instance/${uuid}/eula`,
  });
