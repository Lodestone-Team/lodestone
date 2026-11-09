import { faLink, faUserGroup } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { InstanceShare } from 'bindings/InstanceShare';
import { ShareConnection } from 'bindings/ShareConnection';
import Button from 'components/Atoms/Button';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import Label, { LabelColor } from 'components/Atoms/Label';
import DashboardCard from 'components/DashboardCard';
import {
  instanceShareKey,
  startInstanceShare,
  stopInstanceShare,
  useInstanceShare,
} from 'data/InstanceShare';
import { type MessageKey, useT } from 'i18n';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { errorToString } from 'utils/util';

const CONNECTION_KEYS = {
  same_computer: 'server.share.connection.same_computer',
  local_network: 'server.share.connection.local_network',
  ipv6: 'server.share.connection.ipv6',
  internet: 'server.share.connection.internet',
} as const satisfies Record<ShareConnection, MessageKey>;

/**
 * "Share with friends" on an instance's overview: a link friends open in
 * Lodestone Launcher to join, over a direct connection to this computer
 * (Lodestone Connect). Shown to those who may manage the instance, for
 * Minecraft Java servers, on cores that can share.
 */
export default function ShareWithFriends({
  instance,
}: {
  instance: InstanceInfo;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const isJava = instance.game_type.type === 'MinecraftJava';
  const { data: share, isError } = useInstanceShare(instance.uuid, isJava);
  const [busy, setBusy] = useState(false);
  const [confirmStop, setConfirmStop] = useState(false);

  if (!isJava || isError || !share) return null;

  const update = (next: InstanceShare) =>
    queryClient.setQueryData(instanceShareKey(instance.uuid), next);

  const start = async () => {
    setBusy(true);
    try {
      update(await startInstanceShare(instance.uuid));
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    try {
      update(await stopInstanceShare(instance.uuid));
    } catch (err) {
      toast.error(errorToString(err));
    }
    setConfirmStop(false);
  };

  const copy = (link: string) => {
    // only over HTTPS or on localhost
    navigator.clipboard
      .writeText(link)
      .then(() => toast.info(t('server.share.copied')))
      .catch(() => toast.error(t('server.share.copyFailed')));
  };

  const [stateText, stateColor]: [string, LabelColor] =
    share.state === 'sharing'
      ? [t('server.share.state.sharing'), 'green']
      : share.state === 'reconnecting'
        ? [t('server.share.state.reconnecting'), 'yellow']
        : [t('server.share.state.starting'), 'gray'];

  return (
    <div>
      <h2 className="text-h2 font-extrabold tracking-medium">
        {t('server.share.title')}
      </h2>
      <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
        {t('server.share.lead')}
      </h3>
      <DashboardCard className="mt-4 gap-4">
        {!share.sharing ? (
          <div className="flex flex-row flex-wrap items-center justify-between gap-4">
            <p className="max-w-md text-medium text-gray-300">
              {t('server.share.off')}
            </p>
            <Button
              label={t('server.share.start')}
              intention="primary"
              icon={faLink}
              loading={busy}
              disabled={busy}
              onClick={() => void start()}
            />
          </div>
        ) : (
          <>
            <div className="flex flex-row flex-wrap items-center gap-3">
              <Label size="large" color={stateColor}>
                {stateText}
              </Label>
              {share.stable_link && (
                <span className="text-small font-medium text-gray-500">
                  {t('server.share.stableLink')}
                </span>
              )}
            </div>
            {share.link ? (
              <div className="flex flex-row items-center gap-2">
                <input
                  readOnly
                  value={share.link}
                  aria-label={t('server.share.linkLabel')}
                  onFocus={(e) => e.currentTarget.select()}
                  className="min-w-0 grow truncate rounded border border-gray-faded/30 bg-gray-900 px-2 py-1 font-mono text-small text-gray-300"
                />
                <Button
                  label={t('server.share.copy')}
                  intention="primary"
                  onClick={() => copy(share.link as string)}
                />
              </div>
            ) : (
              <p className="text-medium text-gray-500">
                {t('server.share.preparing')}
              </p>
            )}
            <p className="text-small font-medium text-gray-500">
              {t('server.share.howFriendsJoin')}
            </p>
            {share.problem && (
              <p className="text-small font-medium text-yellow">
                {share.problem}
              </p>
            )}
            {share.hard_network && (
              <p className="text-small font-medium text-yellow">
                {t('server.share.hardNetwork')}
              </p>
            )}
            <div className="flex flex-col gap-2">
              <div className="flex flex-row items-center gap-2 text-medium font-medium text-gray-300">
                <FontAwesomeIcon icon={faUserGroup} className="text-gray-500" />
                {t('server.share.connected', { count: share.friends.length })}
                <span className="text-small text-gray-500">
                  {t('server.share.joins', { count: share.joins })}
                </span>
              </div>
              {share.friends.length > 0 && (
                <ul className="flex flex-col divide-y divide-gray-faded/30 rounded border border-gray-faded/30">
                  {share.friends.map((friend) => (
                    <li
                      key={friend.id}
                      className="flex flex-row flex-wrap items-center justify-between gap-2 px-3 py-2 text-small"
                    >
                      <span className="font-mono text-gray-300">
                        {friend.address}
                      </span>
                      <span className="text-gray-500">
                        {t('server.share.friendLine', {
                          connection: t(CONNECTION_KEYS[friend.connection]),
                          ms: friend.rtt_ms,
                        })}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
            <div className="flex flex-row justify-end">
              <Button
                label={t('server.share.stop')}
                intention="danger"
                onClick={() => setConfirmStop(true)}
              />
            </div>
          </>
        )}
      </DashboardCard>
      <ConfirmDialog
        title={t('server.share.stopTitle')}
        type="danger"
        confirmButtonText={t('server.share.stop')}
        onConfirm={stop}
        onClose={() => setConfirmStop(false)}
        isOpen={confirmStop}
      >
        {t('server.share.stopBody')}
      </ConfirmDialog>
    </div>
  );
}
