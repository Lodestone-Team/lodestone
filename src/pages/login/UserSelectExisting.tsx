import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import { LodestoneContext } from 'data/LodestoneContext';
import { useCoreInfo } from 'data/SystemInfo';
import {
  faArrowLeft,
  faArrowRightArrowLeft,
  faClone,
  faRightFromBracket,
  faUser,
} from '@fortawesome/free-solid-svg-icons';
import { useUid, useUserInfo } from 'data/UserInfo';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import Avatar from 'components/Atoms/Avatar';
import { useDocumentTitle, useEffectOnce } from 'usehooks-ts';
import { tauri } from 'utils/tauriUtil';
import { JwtToken } from 'bindings/JwtToken';
import { isLocalCore } from 'utils/util';

const UserSelectExisting = () => {
  useDocumentTitle('Select user - Lodestone');
  const { setPathname, navigateBack } = useContext(BrowserLocationContext);
  const { setToken, token, core } = useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { data: coreInfo } = useCoreInfo();
  const { core_name, is_setup } = coreInfo ?? {};
  const { data: userInfo, isLoading: isUserInfoLoading } = useUserInfo();
  const uid = useUid();

  useEffectOnce(() => {
    if (token) return;
    if (!tauri || !isLocalCore(core)) {
      setPathname('/login/user', true);
      return;
    }
    tauri
      ?.invoke<JwtToken | null>('get_owner_jwt')
      .then((token) => {
        if (token) {
          setToken(token, socket);
        }
      })
      .catch((err: any) => {
        console.log('Tauri call failed get_owner_jwt', err);
      });
  });

  return (
    <div className="flex w-[640px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl bg-gray-850 px-12 py-14 transition-dimensions @container">
      <div className="text flex flex-col items-start">
        <img src="/logo.svg" alt="logo" className="h-fit w-fit" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          Sign in
        </h1>
        <h2 className="text-h3 font-medium tracking-medium text-white/50">
          {core_name} ({socket})
        </h2>
      </div>
      <div className="flex flex-row items-baseline gap-8">
        {token ? (
          <Button
            type="button"
            iconComponent={<Avatar name={uid} />}
            className="flex-1"
            label={`Continue as ${userInfo?.username ?? 'current user'}`}
            loading={isUserInfoLoading}
            onClick={() => setPathname('/')}
          />
        ) : (
          // TODO: better design and layout in this area
          <Button
            type="button"
            icon={faUser}
            className="flex-1"
            label={`Continue as Current User`}
            disabled={true}
          />
        )}
        <p className="text-medium font-medium tracking-medium text-white/50">
          OR
        </p>
        <Button
          type="button"
          icon={token ? faArrowRightArrowLeft : faRightFromBracket}
          intention={token ? 'info' : 'primary'}
          label={token ? 'Switch user' : 'Sign in'}
          className="flex-1"
          onClick={() => setPathname('/login/user')}
        />
      </div>
      <div className="flex w-full flex-row justify-start gap-4">
        <Button
          type="button"
          icon={faArrowLeft}
          label="Change Core"
          onClick={() => setPathname('/login/core/select')}
        />
      </div>
    </div>
  );
};

export default UserSelectExisting;
