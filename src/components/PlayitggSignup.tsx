import { ReactNode, useEffect } from 'react';
import * as React from 'react';
import { confirmPlayitSignup, generatePlayitSignupLink } from 'utils/apis';
import Button from './Atoms/Button';
import { PlayitSignupData } from 'bindings/PlayitSignupData';
import { SignupStatus } from 'bindings/SignupStatus';

export function PlayitggSignup() {
  const [signupData, setSignupData] = React.useState<PlayitSignupData>({ url: 'link show up here when u click :3', claim_code: "a" });
  const [signupStatus, setSignupStatus] = React.useState<SignupStatus>("CodeNotFound");
  
  const generateLink = async () => {
    setSignupData(await generatePlayitSignupLink());
  }

  const confirmSignup = async () => {
    setSignupStatus(await confirmPlayitSignup(signupData));
  }

  return (
    <div>
     {signupData.url} 
      <Button label="generate link" onClick={generateLink}  />
      <Button label="confirm signup + tunnel creatioj" onClick={confirmSignup}  />
    </div>
  );
}
