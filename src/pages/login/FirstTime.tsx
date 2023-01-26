import Button from 'components/Atoms/Button';
import { useContext, useState } from 'react';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { LodestoneContext } from 'data/LodestoneContext';
import { useDocumentTitle, useEffectOnce } from 'usehooks-ts';
import { tauri } from 'utils/tauriUtil';
import WarningAlert from 'components/Atoms/WarningAlert';
import packageinfo from '../../../package.json';
import { DEFAULT_LOCAL_CORE } from 'utils/util';

const FirstTime = () => {
  useDocumentTitle('Welcome to Lodestone');
  const { setPathname } = useContext(BrowserLocationContext);
  const { coreList, addCore, setCore } = useContext(LodestoneContext);
  const [tauriIsSetup, setTauriIsSetup] = useState(false);

  useEffectOnce(() => {
    if (coreList.length > 0) {
      setPathname('/login/core/select');
      return;
    }
    if (!tauri) return;
    tauri
      .invoke<string | null>('is_setup')
      .then((is_setup) => {
        addCore(DEFAULT_LOCAL_CORE);
        setCore(DEFAULT_LOCAL_CORE);
        setTauriIsSetup(is_setup === 'true');
      })
      .catch((err: any) => {
        console.error('Tauri call failed is_setup', err);
      });
  });

  return (
    <div className="flex w-[680px] max-w-full flex-col items-stretch justify-center gap-16 transition-dimensions @container">
      <div className="flex flex-col items-start gap-4">
        <div className="flex w-full flex-row items-center gap-4">
          <img src="/logo.svg" alt="logo" className="h-8" />
          {/* HACK HACK HACK HACK */}
          <a // solution to get tauri to open links in the browser
            href="https://github.com/Lodestone-Team/lodestone/wiki"
            rel="noreferrer"
            target="_blank"
          >
            <Button // literally just here as a visual component
              label="Wiki & FAQ"
            />
          </a>
        </div>

        <div className="flex flex-row items-start gap-4">
          <h1 className="font-title text-title font-bold tracking-medium text-gray-300">
            Welcome to Lodestone!
          </h1>
          <span className="mt-3 text-right text-small font-medium tracking-medium text-green-200">
            VERSION {packageinfo.version}
          </span>
        </div>

        {!tauri && (
          <WarningAlert>
            <p>
              Lodestone requires mixed/insecure content to be allowed in your
              browser. Learn how to do it{' '}
              <a
                href="https://experienceleague.adobe.com/docs/target/using/experiences/vec/troubleshoot-composer/mixed-content.html?lang=en"
                target="_blank"
                rel="noreferrer"
                className="text-blue-200 underline hover:text-blue-300"
              >
                here.
              </a>
            </p>
          </WarningAlert>
        )}

        <p className="text-medium font-medium tracking-medium text-white">
          Our product is still in its beta release cycle. Browser support is
          limited and bugs are expected. You can check known issues and report
          any new ones on our{' '}
          <a
            href="https://github.com/Lodestone-Team/lodestone/issues"
            target="_blank"
            rel="noreferrer"
            className="text-blue-200 underline hover:text-blue-300"
          >
            Github.
          </a>
        </p>
      </div>

      <div className="flex flex-row flex-wrap items-baseline gap-4">
        {!tauri ? (
          <>
            <Button
              label="Download Lodestone Core"
              onClick={() => {
                window.open(
                  'https://github.com/Lodestone-Team/dashboard/releases/',
                  '_self'
                );
              }}
              intention="primary"
              size="large"
              className="whitespace-nowrap"
            />

            <Button
              label="Connect to existing Core"
              onClick={() => setPathname('/login/core/new')}
              intention="primary"
              size="large"
              className="whitespace-nowrap"
            />
          </>
        ) : tauriIsSetup ? (
          <Button
            label="Continue"
            onClick={() => setPathname('/')}
            intention="primary"
            size="large"
            className="whitespace-nowrap"
          />
        ) : (
          <Button
            label="Setup Lodestone Core"
            onClick={() => setPathname('/login/core/first_setup')}
            intention="primary"
            size="large"
            className="whitespace-nowrap"
          />
        )}
      </div>
    </div>
  );
};

export default FirstTime;
