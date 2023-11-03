import { useContext, useEffect } from 'react';
import * as React from 'react';
import { generatePlayitSignupLink, startCli, stopCli, verifyKey, getTunnels } from 'utils/apis';
import { useEffectOnce } from 'usehooks-ts';
import clsx from 'clsx';
import Button from './Atoms/Button';
import { PlayitSignupData } from 'bindings/PlayitSignupData';
import { PlayitTunnelInfo } from 'bindings/PlayitTunnelInfo';
import IconButton from './Atoms/IconButton';
import { faCarTunnel, faCircle, faCopy, faPowerOff } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { NotificationContext } from 'data/NotificationContext';
import { useQueryClient } from '@tanstack/react-query';
import { TunnelListCard, TunnelListItem } from './TunnelListCard';
import Label, { LabelColor } from './Atoms/Label';
import PlayitggSignupModal from './PlayitggSignupModal';

enum RunnerStatus {
  Stopped = "Stopped",
  Loading = "Loading",
  Started = "Started"
}

export function PlayitggOverview() {
  const [signupData, setSignupData] = React.useState<null | PlayitSignupData>(null);
  const [verified, setVerified] = React.useState<null | boolean>(null);
  const [showSignupModal, setShowSignupModal] = React.useState<boolean>(false);
  const [runnerStatus, setRunnerStatus] = React.useState<RunnerStatus>(RunnerStatus.Stopped);
  const [playitTunnels, setPlayitTunnels] = React.useState<PlayitTunnelInfo[]>([]);
  const queryClient = useQueryClient();
  const { notifications } = useContext(NotificationContext);


  useEffectOnce(() => {
    const inner = async () => {
      setVerified(await verifyKey())
    }
    inner();
  });

  useEffect(() => {
    const inner = async () => {
      const tuns = await getTunnels();
      setPlayitTunnels(tuns);
    }
    inner();
  }, [verified]);

  useEffect(() => {
    const status = queryClient.getQueryData<RunnerStatus>(["playitgg", "status"]);
    setRunnerStatus(status === undefined ? RunnerStatus.Stopped : status);
  }, [notifications, queryClient])

  const generateLink = async () => {
    setSignupData(await generatePlayitSignupLink());
  }

  const handleVerifySignup = async () => {
    setVerified(await verifyKey());
    console.log(notifications)
  }

  let statusColor = "text-gray-faded/30";
  let labelColor = "gray";

  switch (runnerStatus) {
    case RunnerStatus.Started:
      statusColor = 'text-green-300'
      labelColor = 'green'
      break;
    case RunnerStatus.Loading:
      statusColor = 'text-yellow-300'
      labelColor = "yellow"
      break;
  }

  return (
    <div>
      <PlayitggSignupModal modalOpen={showSignupModal} setModalOpen={setShowSignupModal}>
        <h2 className="text-h2 font-extrabold tracking-medium">
          Set Up Playitgg
        </h2>
        {
          signupData === null ?
            <>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                You need to sign up for Playitgg before you can use this feature. Generate a signup link with the button below!
              </h3>
              <Button className='mt-6' label="Generate Link" onClick={generateLink} />
            </>
            :
            <>
              <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
                Follow the link below and make an account! After you make the account and get to the dashboard, press Verify below.
              </h3>
              <a target="_blank" href={signupData.url}>{signupData.url}</a>
              <Button className="mt-6" label="Verify" onClick={() => { handleVerifySignup(); setShowSignupModal(false); }} />
            </>
        }
      </PlayitggSignupModal>
      {verified === null ?
        <h2 className="text-h2 font-extrabold tracking-medium">
          Loading...
        </h2>
        :
        <div>
          <div className='flex flex-row'>
            <h2 className="text-h2 font-extrabold tracking-medium">
              Playitgg Runner
            </h2>
            <IconButton
              icon={faPowerOff}
              onClick={() => {
                if (verified == false) {
                  setShowSignupModal(true)
                } else if (runnerStatus === RunnerStatus.Stopped || runnerStatus === undefined) {
                  startCli(); setRunnerStatus(RunnerStatus.Loading)
                } else {
                  stopCli(); setRunnerStatus(RunnerStatus.Stopped);
                }
              }}
              className='ml-2'
            />
            <Label size="small" color={labelColor as LabelColor} className='ml-2 mt-[6px]'>
              {runnerStatus !== undefined ? runnerStatus : "Stopped"}
            </Label>
          </div>
          <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
            {verified === true ?
              <>If you&apos;re having trouble, make sure your tunnel is set up correctly on the website!</>
              : <>You need to sign up for Playitgg before you can use this. <div className='font-bold hover:cursor-pointer hover:underline' onClick={() => setShowSignupModal(true)}>Press here to sign up.</div></>
            }
          </h3>
          <h2 className="mt-9 text-h2 font-extrabold tracking-medium">
            Tunnels
          </h2>
          <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
            Your tunnels are listed below. You can create more and change the settings on your dashboard on the <a target="_blank" className='underline' href="https://playit.gg/">playit.gg</a> website.
          </h3>
          {playitTunnels.length > 0 ?
            <TunnelListCard className="mt-2">{playitTunnels.map(
              tunnel => (
                <TunnelListItem className='m-2' key={tunnel.server_address} >
                  <div className='flex flex-row'>
                    <h3 className="text-h3 font-bold tracking-medium text-white ">
                      {tunnel.name}
                    </h3>
                    <FontAwesomeIcon
                      icon={faCircle}
                      className={clsx(`ml-2 mt-[9px] select-none text-[9px]`, (tunnel.active && runnerStatus === RunnerStatus.Started) ? 'text-green-300' : `text-gray-faded/30`)}
                    />
                  </div>
                  <h3
                    className="text-h3 font-bold tracking-medium text-white/50 hover:cursor-pointer hover:underline"
                    onClick={() => { navigator.clipboard.writeText(tunnel.server_address) }}
                  >
                    {tunnel.server_address}
                    <FontAwesomeIcon
                      icon={faCopy}
                      className="mx-1 mt-1 h-4 w-4 text-white/50"
                    />
                  </h3>
                  <div className="flex flex-row">
                    <h3 className="text-h3 tracking-medium text-white/50 hover:cursor-pointer">
                      <i>Local server at {tunnel.local_ip}:{tunnel.local_port}</i>
                    </h3>
                  </div>
                </TunnelListItem>
              )
            )}</TunnelListCard>
            :
            <TunnelListCard className="mt-2 border-2 border-dashed p-5">
              <TunnelListItem key="not found">
                <div className='flex flex-row'>
                  <FontAwesomeIcon
                    icon={faCarTunnel}
                    className="mx-1 h-4 w-4 text-white/50"
                  />
                  <div className="mx-1 text-medium italic text-white/50">
                    {verified === true ? "You haven't created any tunnels yet" : "You haven't signed up yet"}
                  </div>
                </div>
              </TunnelListItem>
            </TunnelListCard>
          }
        </div>
      }
    </div >
  );
}
