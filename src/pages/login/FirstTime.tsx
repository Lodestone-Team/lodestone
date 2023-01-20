import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import {
  axiosPutSingleValue,
  DEFAULT_LOCAL_CORE,
  DISABLE_AUTOFILL,
  errorToString,
} from 'utils/util';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import * as yup from 'yup';
import { faArrowLeft, faArrowRight } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { useQueryClient } from '@tanstack/react-query';
import { useGlobalSettings } from 'data/GlobalSettings';
import { LodestoneContext } from 'data/LodestoneContext';
import { useCoreInfo } from 'data/SystemInfo';
import { useEffectOnce } from 'usehooks-ts';
import { tauri } from 'utils/tauriUtil';

const FirstTime = () => {
  const { setPathname } = useContext(BrowserLocationContext);
  const { coreList, addCore, setCore } = useContext(LodestoneContext);
  const queryClient = useQueryClient();
  const { data: globalSettings, isLoading, error } = useGlobalSettings();

  useEffectOnce(() => {
    if (coreList.length > 0) {
      setPathname('/login/core/select');
      return;
    }
    if (!tauri) return;
    tauri
      ?.invoke<string | null>('is_setup')
      .then((is_setup) => {
        addCore(DEFAULT_LOCAL_CORE);
        setCore(DEFAULT_LOCAL_CORE);
        if (is_setup) {
          setPathname('/');
        } else {
          setPathname('/login/core/first_setup');
        }
      })
      .catch((err: any) => {
        console.log('Tauri call failed is_setup', err);
      });
  });

  return (
    <div className="flex w-[680px] max-w-full flex-col items-stretch justify-center gap-16 rounded-2xl px-12 py-16 transition-dimensions @container">
      <div className="text flex flex-col items-start gap-4">
        <div className="flex w-full flex-row items-center gap-4">
          <img src="/logo.svg" alt="logo" className="h-8" />
          <Button
            label="Wiki & FAQ"
            onClick={() => {
              window.open(
                'https://github.com/Lodestone-Team/lodestone/wiki',
                '_self'
              );
            }}
          />
        </div>

        <div className="flex flex-row items-start gap-4">
          <h1 className="font-title text-title font-bold tracking-medium text-gray-300">
            Welcome to Lodestone!
          </h1>
          <span className="mt-3 text-right text-small font-medium tracking-medium text-green-200">
            BETA 0.4.1 WEB
          </span>
        </div>

        <p className="rounded-md border border-red-200 bg-red-faded/25 p-2 text-medium font-medium tracking-medium text-white">
          Lodestone requires mixed/insecure content to be allowed in your
          browser. Learn how to do it{' '}
          <a
            href="https://github.com/Lodestone-Team/lodestone/wiki"
            target="_blank"
            rel="noreferrer"
            className="text-blue-200 underline hover:text-blue-300"
          >
            here.
          </a>
        </p>

        <p className="text-medium font-medium tracking-medium text-white">
          Our product is still in its beta release cycle. Browser support is
          limited and bugs are expected. You can check known issues and report
          any new ones on our {' '}
          <a
            href="https://github.com/Lodestone-Team/lodestone"
            target="_blank"
            rel="noreferrer"
            className="text-blue-200 underline hover:text-blue-300"
          >
            Github.
          </a>
        </p>
      </div>

      <div className="flex flex-row items-baseline gap-4">
        <Button
          label="Download Lodestone Core"
          onClick={() => {
            window.open(
              'https://github.com/Lodestone-Team/dashboard/releases/',
              '_self'
            );
          }}
          color="primary"
          size="large"
        />

        <Button
          label="Connect to existing Core"
          onClick={() => setPathname('/login/core/new')}
          color="primary"
          size="large"
        />
      </div>
    </div>
  );
};

export default FirstTime;
