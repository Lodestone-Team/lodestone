import Button from 'components/Atoms/Button';
import { useContext, useState } from 'react';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { LodestoneContext } from 'data/LodestoneContext';
import { useDocumentTitle } from 'usehooks-ts';
import { useEffectOnce } from 'utils/hooks';
import { tauri } from 'utils/tauriUtil';
import WarningAlert from 'components/Atoms/WarningAlert';
import { DASHBOARD_VERSION } from 'utils/version';
import { DEFAULT_LOCAL_CORE } from 'utils/util';
import Logo from 'components/Atoms/Logo';
import { useI18n } from 'i18n';

const FirstTime = () => {
  const { t, rich } = useI18n();
  useDocumentTitle(t('shell.welcome.pageTitle'));
  const { setPathname } = useContext(BrowserLocationContext);
  const { coreList, addCore, setCore } = useContext(LodestoneContext);
  // null until the desktop app has said whether its core is set up
  const [tauriIsSetup, setTauriIsSetup] = useState<boolean | null>(null);
  // Cores known when the page opens: it only redirects (below), so don't
  // flash it first. Read once: the desktop app adds its core further down.
  const [redirecting] = useState(() => coreList.length > 0);

  useEffectOnce(() => {
    if (coreList.length > 0) {
      setPathname('/login/core/select');
      return;
    }
    if (!tauri) return;
    tauri.core
      .invoke<string | null>('is_setup')
      .then((is_setup) => {
        addCore(DEFAULT_LOCAL_CORE);
        setCore(DEFAULT_LOCAL_CORE);
        setTauriIsSetup(is_setup === 'true');
      })
      .catch((err: any) => {
        console.error('Tauri call failed is_setup', err);
        setTauriIsSetup(false);
      });
  });

  if (redirecting) return null;

  return (
    <div className="@container flex w-[680px] max-w-full flex-col items-stretch justify-center gap-16 transition-dimensions">
      <div className="flex flex-col items-start gap-4">
        <div className="flex w-full flex-row items-center gap-4">
          <Logo alt="Lodestone" className="h-8" />
          {/* HACK HACK HACK HACK */}
          <a // solution to get tauri to open links in the browser
            href="https://github.com/Lodestone-Team/lodestone/wiki"
            rel="noreferrer"
            target="_blank"
          >
            <Button // literally just here as a visual component
              label={t('shell.welcome.wiki')}
            />
          </a>
        </div>

        <div className="flex flex-row items-start gap-4">
          <h1 className="font-title text-title font-bold tracking-medium text-gray-300">
            {t('shell.welcome.title')}
          </h1>
          <span className="mt-3 text-right text-small font-medium tracking-medium text-green-200">
            {t('shell.welcome.version', { version: DASHBOARD_VERSION })}
          </span>
        </div>

        <p className="text-medium font-medium tracking-medium text-white">
          <span className="font-extrabold text-red-200">
            {t('shell.welcome.beta')}
          </span>{' '}
          {rich('shell.welcome.betaBody', {
            github: (
              <a
                href="https://github.com/Lodestone-Team/lodestone/issues"
                target="_blank"
                rel="noreferrer"
                className="text-blue-200 underline hover:text-blue-300"
              >
                Github
              </a>
            ),
          })}
        </p>
      </div>

      <div className="flex flex-row flex-wrap items-baseline gap-4">
        {!tauri ? (
          <>
            <Button
              label={t('shell.login.downloadCore')}
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
              label={t('shell.welcome.connectExisting')}
              onClick={() => setPathname('/login/core/new')}
              intention="primary"
              size="large"
              className="whitespace-nowrap"
            />
          </>
        ) : tauriIsSetup === null ? (
          // the button's place, until it is known which one it is (it used
          // to show "Setup Lodestone Core" first, then switch to "Continue")
          <Button
            label={t('shell.welcome.setupCore')}
            intention="primary"
            size="large"
            className="invisible whitespace-nowrap"
            disabled
          />
        ) : tauriIsSetup ? (
          <Button
            label={t('shell.login.continue')}
            onClick={() => setPathname('/')}
            intention="primary"
            size="large"
            className="whitespace-nowrap"
          />
        ) : (
          <Button
            label={t('shell.welcome.setupCore')}
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
