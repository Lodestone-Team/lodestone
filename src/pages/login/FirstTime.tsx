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
    <div className="flex w-[640px] max-w-full flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-850 px-12 py-12 transition-dimensions @container">
      <div className="text flex flex-col items-start">
        <img src="/logo.svg" alt="logo" className="h-9 w-40" />
        <h1 className="font-title text-2xlarge font-medium-semi-bold tracking-medium text-gray-300">
          Welcome to Lodestone
        </h1>
      </div>

      <div className="flex flex-row items-baseline gap-8">
        <Button
          className="flex-1"
          label="Download Lodestone Core"
          onClick={() => {
            window.open(
              'https://github.com/Lodestone-Team/dashboard/releases/',
              '_self'
            );
          }}
        />
        <p>OR</p>
        <Button
          className="flex-1"
          label="Connect to existing Core"
          onClick={() => setPathname('/login/core/new')}
        />
      </div>
    </div>
  );
};

export default FirstTime;
