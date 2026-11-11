import { useQuery } from '@tanstack/react-query';
import { InstanceShare } from 'bindings/InstanceShare';
import { InstanceUuid } from 'bindings/InstanceUuid';
import { axiosWrapper } from 'utils/util';

/** The query key of an instance's sharing status. */
export const instanceShareKey = (uuid: InstanceUuid) => [
  'instances',
  uuid,
  'share',
];

/**
 * Whether an instance is shared with a link through Lodestone Connect, its
 * link and who is connected. Checked every few seconds while it is shared.
 * Fails (and stays failed) for users who can't manage the instance and on
 * cores without sharing.
 */
export const useInstanceShare = (uuid: InstanceUuid, enabled: boolean) =>
  useQuery({
    queryKey: instanceShareKey(uuid),
    queryFn: () =>
      axiosWrapper<InstanceShare>({
        method: 'get',
        url: `/instance/${uuid}/share`,
      }),
    enabled,
    retry: false,
    refetchInterval: (query) => {
      const share = query.state.data;
      if (!share?.sharing) return false;
      return share.state === 'sharing' ? 5000 : 2000;
    },
  });

/** Starts sharing; answers once the link is known (a few seconds). */
export const startInstanceShare = (uuid: InstanceUuid) =>
  axiosWrapper<InstanceShare>({
    method: 'post',
    url: `/instance/${uuid}/share`,
  });

/** Stops sharing. The link stops working for good. */
export const stopInstanceShare = (uuid: InstanceUuid) =>
  axiosWrapper<InstanceShare>({
    method: 'delete',
    url: `/instance/${uuid}/share`,
  });
