import { ReactNode, useEffect } from 'react';
import * as React from 'react';
import { confirmPlayitSignup, generatePlayitSignupLink, startTunnel, stopTunnel } from 'utils/apis';
import Button from './Atoms/Button';
import { PlayitSignupData } from 'bindings/PlayitSignupData';
import { SignupStatus } from 'bindings/SignupStatus';
import { PlayitTunnelInfo } from 'bindings/PlayitTunnelInfo';
import { PortType } from 'bindings/PortType';


export function TunnelList() {
  const [portType, setPortType] = React.useState<string>("");
  const [port, setPort] = React.useState<number | unknown>();
  const [tunnel, setTunnel] = React.useState<PlayitTunnelInfo>();

  const handleStartTunnel = async () => {
    setTunnel(await startTunnel({ local_port: port as number, port_type: portType as PortType}));
  }
  
  return (
    <div>
      <Button label="start tunnel" 
        onClick={handleStartTunnel}
      />
      <Button label="stop tunnel" 
        onClick={() => stopTunnel(tunnel as PlayitTunnelInfo)}
       />
      <input
          placeholder='enter port u r running server on :3'
          onChange={(e) => setPort(parseInt(e.target.value) as (number | unknown))}
      />
      <br/>
      <input
          placeholder='enter port type :3 (if minecraft just do tcp :D)'
          onChange={(e) => setPortType(e.target.value)}
      />
    </div>
  );
}
