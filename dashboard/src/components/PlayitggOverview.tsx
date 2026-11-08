import { useEffect } from 'react';
import * as React from 'react';
import {
  cliIsRunning,
  generatePlayitSignupLink,
  getTunnels,
  startCli,
  stopCli,
  verifyKey,
} from 'utils/apis';
import { errorToString } from 'utils/util';
import { toast } from 'react-toastify';
import clsx from 'clsx';
import Button from './Atoms/Button';
import { PlayitSignupData } from 'bindings/PlayitSignupData';
import { PlayitTunnelInfo } from 'bindings/PlayitTunnelInfo';
import IconButton from './Atoms/IconButton';
import {
  faCarTunnel,
  faCircle,
  faCopy,
  faPowerOff,
  faRefresh,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { TunnelListCard, TunnelListItem } from './TunnelListCard';
import Label, { LabelColor } from './Atoms/Label';
import PlayitggSignupModal from './PlayitggSignupModal';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { MessageKey, useI18n } from 'i18n';

enum RunnerStatus {
  Stopped = 'Stopped',
  Loading = 'Loading',
  Started = 'Started',
}

const runnerStatusLabels = {
  [RunnerStatus.Stopped]: 'console.playit.runner.Stopped',
  [RunnerStatus.Loading]: 'console.playit.runner.Loading',
  [RunnerStatus.Started]: 'console.playit.runner.Started',
} as const satisfies Record<RunnerStatus, MessageKey>;

const RUNNER_STATUS_KEY = ['playitgg', 'status'];

const VERIFIED_KEY = ['playitgg', 'verified'];
const TUNNELS_KEY = ['playitgg', 'tunnels'];

// tunnel entries: name, address, local server
const TunnelsSkeleton = () => (
  <TunnelListCard className="mt-2">
    {[0, 1].map((i) => (
      <TunnelListItem className="m-2 flex flex-col gap-2" key={i}>
        <Skeleton className="h-5 w-40" />
        <Skeleton className="h-5 w-64 max-w-full" />
        <Skeleton className="h-4 w-48 rounded-sm" />
      </TunnelListItem>
    ))}
  </TunnelListCard>
);

export function PlayitggOverview() {
  const [signupData, setSignupData] = React.useState<null | PlayitSignupData>(
    null
  );
  const [showSignupModal, setShowSignupModal] = React.useState<boolean>(false);
  const queryClient = useQueryClient();
  const { t, rich } = useI18n();
  // fetched when the page opens, then kept up to date by the event stream
  const runnerQuery = useQuery<RunnerStatus>({
    queryKey: RUNNER_STATUS_KEY,
    queryFn: async () =>
      (await cliIsRunning()) ? RunnerStatus.Started : RunnerStatus.Stopped,
  });
  const runnerStatus = runnerQuery.data ?? RunnerStatus.Stopped;
  const setRunnerStatus = (status: RunnerStatus) =>
    queryClient.setQueryData(RUNNER_STATUS_KEY, status);

  // Both are fetched again every time the page opens, but kept in between:
  // reopening the page shows the last result at once instead of "Loading..."
  const verifiedQuery = useQuery<boolean>({
    queryKey: VERIFIED_KEY,
    queryFn: verifyKey,
    staleTime: 0,
    retry: false,
  });
  const verified = verifiedQuery.data ?? null;
  const verifyError =
    verified === null && verifiedQuery.error
      ? errorToString(verifiedQuery.error)
      : null;
  const tunnelsQuery = useQuery<PlayitTunnelInfo[]>({
    queryKey: TUNNELS_KEY,
    // no tunnels if they can't be fetched (e.g. not signed up yet)
    queryFn: () => getTunnels().catch(() => []),
    staleTime: 0,
  });
  const playitTunnels = tunnelsQuery.data ?? [];
  // only the first load: a refresh keeps the list (the icon spins instead)
  const loadingTunnels = tunnelsQuery.isPending;
  const loadTunnels = () => void tunnelsQuery.refetch();

  useEffect(() => {
    if (verified === false) {
      setShowSignupModal(true);
    }
  }, [verified]);

  const generateLink = async () => {
    try {
      setSignupData(await generatePlayitSignupLink());
    } catch (e) {
      toast.error(
        t('console.playit.signupFailed', { error: errorToString(e) })
      );
    }
  };

  const handleVerifySignup = async () => {
    try {
      queryClient.setQueryData(VERIFIED_KEY, await verifyKey());
      // the tunnels of the account just signed up to
      void queryClient.invalidateQueries({ queryKey: TUNNELS_KEY });
    } catch (e) {
      toast.error(
        t('console.playit.verifyFailed', { error: errorToString(e) })
      );
    }
  };

  const toggleRunner = async () => {
    if (runnerStatus === RunnerStatus.Stopped) {
      const error = await startCli();
      if (error) toast.error(t('console.playit.startFailed', { error }));
      else setRunnerStatus(RunnerStatus.Loading);
    } else {
      const error = await stopCli();
      if (error) toast.error(t('console.playit.stopFailed', { error }));
      else setRunnerStatus(RunnerStatus.Stopped);
    }
  };

  let statusColor = 'text-gray-faded/30';
  let labelColor = 'gray';

  switch (runnerStatus) {
    case RunnerStatus.Started:
      statusColor = 'text-green-300';
      labelColor = 'green';
      break;
    case RunnerStatus.Loading:
      statusColor = 'text-yellow-300';
      labelColor = 'yellow';
      break;
  }

  return (
    <div>
      <PlayitggSignupModal
        modalOpen={showSignupModal}
        setModalOpen={setShowSignupModal}
      >
        <h2 className="text-h2 font-extrabold tracking-medium">
          {t('console.playit.setupTitle')}
        </h2>
        {signupData === null ? (
          <>
            <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
              {t('console.playit.setupIntro')}
            </h3>
            <Button
              className="mt-6"
              label={t('console.playit.generateLink')}
              onClick={generateLink}
            />
          </>
        ) : (
          <>
            <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
              {t('console.playit.setupFollowLink')}
            </h3>
            <a target="_blank" href={signupData.url}>
              {signupData.url}
            </a>
            <Button
              className="mt-6"
              label={t('console.playit.verify')}
              onClick={() => {
                handleVerifySignup();
                setShowSignupModal(false);
              }}
            />
          </>
        )}
      </PlayitggSignupModal>
      {verified === null ? (
        verifyError === null ? (
          // the page's shape while the status loads (after a moment)
          <SkeletonGroup label={t('console.playit.loading')}>
            <div className="flex flex-row items-center gap-2">
              <Skeleton className="h-7 w-52" />
              <Skeleton className="size-8" />
              <Skeleton className="h-5 w-16 rounded-full" />
            </div>
            <Skeleton className="mt-2 h-4 w-96 max-w-full rounded-sm" />
            <Skeleton className="mt-9 h-7 w-28" />
            <Skeleton className="mt-2 h-4 w-80 max-w-full rounded-sm" />
            <TunnelsSkeleton />
          </SkeletonGroup>
        ) : (
          <h2 className="text-h2 font-extrabold tracking-medium">
            {t('console.playit.loadFailed', { error: verifyError })}
          </h2>
        )
      ) : (
        <div>
          <div className="flex flex-row">
            <h2 className="text-h2 font-extrabold tracking-medium">
              {t('console.playit.runnerTitle')}
            </h2>
            <IconButton
              icon={faPowerOff}
              onClick={() => {
                if (verified == false) {
                  setShowSignupModal(true);
                } else {
                  toggleRunner();
                }
              }}
              className="ml-2"
              disabled={!verified}
            />
            {runnerQuery.isPending ? (
              // no "Stopped" shown before the status is known
              <SkeletonGroup className="mt-[6px] ml-2">
                <Skeleton className="h-5 w-16 rounded-full" />
              </SkeletonGroup>
            ) : (
              <Label
                size="small"
                color={labelColor as LabelColor}
                className="mt-[6px] ml-2"
              >
                {t(runnerStatusLabels[runnerStatus])}
              </Label>
            )}
          </div>
          <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
            {verified === true ? (
              rich('console.playit.trouble', {
                running: <b>{t('console.playit.troubleRunning')}</b>,
              })
            ) : (
              <>
                {t('console.playit.signupRequired')}{' '}
                <div
                  className="font-bold hover:cursor-pointer hover:underline"
                  onClick={() => setShowSignupModal(true)}
                >
                  {t('console.playit.signupLink')}
                </div>
              </>
            )}
          </h3>
          <div className="flex flex-row">
            <h2 className="mt-9 text-h2 font-extrabold tracking-medium">
              {t('console.playit.tunnels')}
            </h2>
            <FontAwesomeIcon
              icon={faRefresh}
              className={clsx(
                'mx-2 mt-11 mb-0 size-4 text-white/50 hover:cursor-pointer',
                tunnelsQuery.isFetching && !loadingTunnels && 'animate-spin'
              )}
              onClick={() => loadTunnels()}
              title={t('console.playit.refreshTunnels')}
            />
          </div>
          <h3 className="text-h3 font-medium tracking-medium text-white/50 italic">
            {rich('console.playit.tunnelsIntro', {
              link: (
                <a
                  target="_blank"
                  className="underline"
                  href="https://playit.gg/"
                >
                  playit.gg
                </a>
              ),
            })}
          </h3>
          {loadingTunnels ? (
            <SkeletonGroup label={t('console.playit.loadingTunnels')}>
              <TunnelsSkeleton />
            </SkeletonGroup>
          ) : playitTunnels.length > 0 ? (
            <TunnelListCard className="mt-2">
              {playitTunnels.map((tunnel) => (
                <TunnelListItem className="m-2" key={tunnel.server_address}>
                  <div className="flex flex-row">
                    <h3 className="text-h3 font-bold tracking-medium text-white">
                      {tunnel.name}
                    </h3>
                    <FontAwesomeIcon
                      icon={faCircle}
                      className={clsx(
                        `mt-[9px] ml-2 text-[9px] select-none`,
                        tunnel.active && runnerStatus === RunnerStatus.Started
                          ? 'text-green-300'
                          : `text-gray-faded/30`
                      )}
                    />
                  </div>
                  <h3
                    className="text-h3 font-bold tracking-medium text-white/50 hover:cursor-pointer hover:underline"
                    onClick={() => {
                      navigator.clipboard.writeText(tunnel.server_address);
                    }}
                    title={t('console.playit.copyAddress')}
                  >
                    {tunnel.server_address}
                    <FontAwesomeIcon
                      icon={faCopy}
                      className="mx-1 mt-1 h-4 w-4 text-white/50"
                    />
                  </h3>
                  <div className="flex flex-row">
                    <h3 className="text-h3 tracking-medium text-white/50 hover:cursor-pointer">
                      <i>
                        {t('console.playit.localServer', {
                          address: `${tunnel.local_ip}:${tunnel.local_port}`,
                        })}
                      </i>
                    </h3>
                  </div>
                </TunnelListItem>
              ))}
            </TunnelListCard>
          ) : (
            <TunnelListCard className="mt-2 border-2 border-dashed p-5">
              <TunnelListItem key="not found">
                <div className="flex flex-row">
                  <FontAwesomeIcon
                    icon={faCarTunnel}
                    className="mx-1 h-4 w-4 text-white/50"
                  />
                  <div className="mx-1 text-medium text-white/50 italic">
                    {verified === true
                      ? t('console.playit.noTunnels')
                      : t('console.playit.notSignedUp')}
                  </div>
                </div>
              </TunnelListItem>
            </TunnelListCard>
          )}
          <a
            className="mt-6 block text-h3 font-medium tracking-medium text-white/50 hover:cursor-pointer hover:underline"
            href="https://github.com/Lodestone-Team/lodestone/wiki/Playit.gg-Integration"
          >
            {t('console.playit.help')}
          </a>
        </div>
      )}
    </div>
  );
}
