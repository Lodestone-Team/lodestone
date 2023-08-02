import { ReactNode, useEffect } from 'react';
import * as React from 'react';
import { generatePlayitSignupLink, startCli, stopCli, verifyKey  } from 'utils/apis';
import { useEffectOnce } from 'usehooks-ts';
import Button from './Atoms/Button';
import { PlayitSignupData } from 'bindings/PlayitSignupData';
import { SignupStatus } from 'bindings/SignupStatus';

export function PlayitggSignup() {
  const [signupData, setSignupData] = React.useState<null | PlayitSignupData>(null);
  const [verified, setVerified] = React.useState<null | boolean>(null);
  const [signupStatus, setSignupStatus] = React.useState<SignupStatus>("CodeNotFound");
  const [cliRunning, setCliRunning] = React.useState<null | boolean>(false);

  useEffectOnce(() => {
    const inner = async () => { 
      setVerified(await verifyKey())
    } 
    inner();
  });
  
  const generateLink = async () => {
    setSignupData(await generatePlayitSignupLink());
  }

  const handleVerifySignup = async () => {
    setVerified(await verifyKey());
  }

  return (
    
    <div>
      { verified === null ? <div> loading... </div> 
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
                <Button label="Generate Link" onClick={generateLink}  />
              :
                <div>
                  <a href={signupData.url}>{signupData.url}</a>
                  <Button label="Verify" onClick={handleVerifySignup}  />
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
                !cliRunning ?
                  <Button label="Start" onClick={startCli}/>
                  :
                  <Button label="Stop" onClick={stopCli}/>
              }
            </div>
          </div>
      }
    </div>
  );
}
