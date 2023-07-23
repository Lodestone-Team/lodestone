import { ReactNode, useEffect } from 'react';
import * as React from 'react';
import { generatePlayitSignupLink, startCli, stopCli } from 'utils/apis';
import Button from './Atoms/Button';
import { PlayitSignupData } from 'bindings/PlayitSignupData';
import { SignupStatus } from 'bindings/SignupStatus';

export function PlayitggSignup() {
  const [signupData, setSignupData] = React.useState<PlayitSignupData>({ url: 'link show up here when u click :3', claim_code: "a" });
  const [signupStatus, setSignupStatus] = React.useState<SignupStatus>("CodeNotFound");
  
  const generateLink = async () => {
    setSignupData(await generatePlayitSignupLink());
  }

  const handleStartCli = async () => {
    await startCli();
  }
  
  const handleStopCli = async () => {
    await stopCli();
  }

  return (
    <div>
     {signupData.url} 
      <Button label="generate link" onClick={generateLink}  />
      <Button label="start cli" onClick={startCli}/>
      <Button label="stop cli" onClick={stopCli}/>
    </div>
  );
}
