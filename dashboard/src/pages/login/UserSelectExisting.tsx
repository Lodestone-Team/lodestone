import Button from 'components/Atoms/Button';
import { useContext, useState } from 'react';
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
import { useDocumentTitle } from 'usehooks-ts';
import { useEffectOnce } from 'utils/hooks';
import { tauri } from 'utils/tauriUtil';
import { JwtToken } from 'bindings/JwtToken';
import { isLocalCore } from 'utils/util';
import Logo from 'components/Atoms/Logo';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { useT } from 'i18n';

const UserSelectExisting = () => {
  const t = useT();
  useDocumentTitle(
    t('common.pageTitle', { page: t('shell.userSelect.pageTitle') })
  );
  const { setPathname, navigateBack } = useContext(BrowserLocationContext);
  const { setToken, token, core } = useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { data: coreInfo } = useCoreInfo();
  const { core_name, is_setup } = coreInfo ?? {};
  const { data: userInfo, isPending: isUserInfoLoading } = useUserInfo();
  const uid = useUid();
  // Signed out, outside the desktop app's own core: this page only sends the
  // user on to the sign-in form (below). Render nothing meanwhile instead of
  // flashing it. Read once, like the redirect.
  const [redirecting] = useState(
    () => !token && (!tauri || !isLocalCore(core))
  );

  useEffectOnce(() => {
    if (token) return;
    if (!tauri || !isLocalCore(core)) {
      setPathname('/login/user', true);
      return;
    }
    tauri?.core
      .invoke<JwtToken | null>('get_owner_jwt')
      .then((token) => {
        if (token) {
          setToken(token, socket);
        }
      })
      .catch((err: any) => {
        console.log('Tauri call failed get_owner_jwt', err);
      });
  });

  if (redirecting) return null;

  return (
    <div className="@container flex w-[640px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl bg-gray-850 px-12 py-14 transition-dimensions">
      <div className="flex flex-col items-start">
        <Logo alt="Lodestone" className="h-8" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          {t('shell.login.signIn')}
        </h1>
        <h2 className="text-h3 font-medium tracking-medium text-white/50">
          {core_name} ({socket})
        </h2>
      </div>
      <div className="flex flex-row items-baseline gap-8">
        {token && isUserInfoLoading ? (
          // the button's shape until the user's name is known
          <SkeletonGroup
            label={t('shell.userSelect.loadingUser')}
            className="flex-1"
          >
            <Skeleton className="h-8 w-full rounded" />
          </SkeletonGroup>
        ) : token ? (
          <Button
            type="button"
            iconComponent={<Avatar name={uid} />}
            className="flex-1"
            label={
              userInfo
                ? t('shell.userSelect.continueAs', { name: userInfo.username })
                : t('shell.userSelect.continueAsCurrent')
            }
            onClick={() => setPathname('/')}
          />
        ) : (
          // TODO: better design and layout in this area
          <Button
            type="button"
            icon={faUser}
            className="flex-1"
            label={t('shell.userSelect.continueAsCurrent')}
            disabled={true}
          />
        )}
        <p className="text-medium font-medium tracking-medium text-white/50">
          {t('shell.login.or')}
        </p>
        <Button
          type="button"
          icon={token ? faArrowRightArrowLeft : faRightFromBracket}
          intention={token ? 'info' : 'primary'}
          label={
            token ? t('shell.userSelect.switchUser') : t('shell.login.signIn')
          }
          className="flex-1"
          onClick={() => setPathname('/login/user')}
        />
      </div>
      <div className="flex w-full flex-row justify-start gap-4">
        <Button
          type="button"
          icon={faArrowLeft}
          label={t('shell.login.changeCore')}
          onClick={() => setPathname('/login/core/select')}
        />
      </div>
    </div>
  );
};

export default UserSelectExisting;
