import { ReactNode, useContext, useEffect } from 'react';
import * as React from 'react';
import { generatePlayitSignupLink, startCli, stopCli, verifyKey, cliIsRunning, getTunnels } from 'utils/apis';
import { useEffectOnce } from 'usehooks-ts';
import clsx from 'clsx';
import Button from './Atoms/Button';
import { PlayitSignupData } from 'bindings/PlayitSignupData';
import { SignupStatus } from 'bindings/SignupStatus';
import { PlayitTunnelInfo } from 'bindings/PlayitTunnelInfo';
import IconButton from './Atoms/IconButton';
import { faCircle, faCopy } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { NotificationContext } from 'data/NotificationContext';
import { useQueryClient } from '@tanstack/react-query';

enum RunnerStatus {
  Stopped = "Stopped",
  Loading = "Loading",
  Started = "Started"
}

export function PlayitggSignup() {
  const [signupData, setSignupData] = React.useState<null | PlayitSignupData>(null);
  const [verified, setVerified] = React.useState<null | boolean>(null);
  const [runnerStatus, setRunnerStatus] = React.useState<RunnerStatus | undefined>(RunnerStatus.Stopped);
  const [signupStatus, setSignupStatus] = React.useState<SignupStatus>("CodeNotFound");
  const [playitTunnels, setPlayitTunnels] = React.useState<PlayitTunnelInfo[]>([]);
  const queryClient = useQueryClient();
  const { notifications, ongoingNotifications } = useContext(NotificationContext);


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
    setRunnerStatus(queryClient.getQueryData<RunnerStatus>(["playitgg", "status"]));
  }, [notifications])

  const generateLink = async () => {
    setSignupData(await generatePlayitSignupLink());
  }

  const handleVerifySignup = async () => {
    setVerified(await verifyKey());
    console.log(notifications)
  }

  return (

    <div>
      {verified === null ? <div> loading... </div>
        :
        (verified === false || verified === null) ?
          <div>
            <h2 className="text-h2 font-extrabold tracking-medium">
              Get Playitgg set up!
            </h2>
            <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
              Follow the link below and make an account!
            </h3>
            {
              signupData === null ?
                <Button label="Generate Link" onClick={generateLink} />
                :
                <div>
                  <a target="_blank" href={signupData.url}>{signupData.url}</a>
                  <Button label="Verify" onClick={handleVerifySignup} />
                </div>
            }
          </div>
          :
          <div>
            <h2 className="text-h2 font-extrabold tracking-medium">
              Start Playitgg and get your server online!
            </h2>
            <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
              If you're having trouble, make sure your tunnel is set up correctly on the website!
            </h3>
            <div className="flex row">
              {
                (runnerStatus === RunnerStatus.Stopped || runnerStatus === undefined)?
                  <Button label="Start" onClick={() => { startCli(); setRunnerStatus(RunnerStatus.Loading); }} />
                  :
                  <Button label="Stop" onClick={() => { stopCli(); setRunnerStatus(RunnerStatus.Stopped); }} />
              }
              <FontAwesomeIcon
                icon={faCircle}
                className={clsx(
                  `select-none ml-2 text-[9px] mt-[9px] text-gray-faded/30`,
                  runnerStatus === RunnerStatus.Started && 'text-green-300', 
                  runnerStatus === RunnerStatus.Loading && `text-yellow-300`)}
              />
            </div>
            <div className="mt-3">{playitTunnels.length > 0 && playitTunnels.map(
              tunnel => (
                <div>
                  <div className='flex row'>
                    <h3 className="text-h3 font-medium font-bold tracking-medium text-white">
                      {tunnel.name}
                    </h3>
                    <FontAwesomeIcon
                      icon={faCircle}
                      className={clsx(`select-none ml-2 text-[9px] mt-[9px]`, (tunnel.active && runnerStatus === RunnerStatus.Started) ? 'text-green-300' : `text-gray-faded/30`)}
                    />
                  </div>
                  <div className="flex row">
                    <h3
                      className="hover:cursor-pointer hover:underline text-h3 font-small font-bold tracking-medium text-white/50"
                      onClick={() => { navigator.clipboard.writeText(tunnel.server_address) }}
                    >
                      {tunnel.server_address}
                    </h3>
                    <IconButton type="button" className="ml-2" icon={faCopy}
                      onClick={() => { navigator.clipboard.writeText(tunnel.server_address) }}
                    />
                  </div>
                  <div className="flex row">
                    <h3 className="hover:cursor-pointer hover:underline text-h3 font-small font-bold tracking-medium text-white/50">
                      Local server at {tunnel.local_ip}:{tunnel.local_port}
                    </h3>
                  </div>
                </div>
              )
            )}</div>
          </div>
      }
    </div>
  );
}
