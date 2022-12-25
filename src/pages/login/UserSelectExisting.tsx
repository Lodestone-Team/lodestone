import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import { LodestoneContext } from 'data/LodestoneContext';
import { useCoreInfo } from 'data/SystemInfo';
import {
  faArrowLeft,
  faClone,
  faUser,
} from '@fortawesome/free-solid-svg-icons';
import { useUserInfo } from 'data/UserInfo';
import { BrowserLocationContext } from 'data/BrowserLocationContext';

const UserSelectExisting = () => {
  const { setPathname } = useContext(BrowserLocationContext);
  const { setToken, core } = useContext(LodestoneContext);
  const { address, port } = core;
  const socket = `${address}:${port}`;
  const { data: coreInfo } = useCoreInfo();
  const { core_name } = coreInfo ?? {};
  const { data: userInfo } = useUserInfo();

  return (
    <div
      className="flex h-screen flex-col items-center justify-center p-8"
      style={{
        background: "url('/login_background.svg')",
        backgroundSize: 'cover',
      }}
    >
      <div className="flex w-[768px] max-w-full flex-col items-stretch justify-center gap-12 rounded-3xl bg-gray-850 px-14 py-20 @container">
        <div className="text flex flex-col items-start">
          <h1 className=" font-title text-2xlarge font-medium tracking-medium text-gray-300">
            Sign-in to {core_name ?? '...'}
          </h1>
          <h2 className="h-9 text-medium font-medium tracking-medium text-gray-300">
            Base URL: {socket}
          </h2>
        </div>
        <div className="flex h-32 flex-row items-baseline gap-8">
          {userInfo?.username ? (
            <Button
              icon={faUser}
              label={`Log in as ${userInfo?.username ?? '...'}`}
              onClick={() => setPathname('/')}
            />
          ) : (
            <Button
              icon={faUser}
              label={`Log in as guest`}
              onClick={() => {
                setToken('', socket);
                setPathname('/');
              }}
            />
          )}
          <p>OR</p>
          <Button
            icon={faClone}
            label="Login as another user"
            className=""
            onClick={() => setPathname('/login/user/new')}
          />
        </div>
        <div className="flex w-full flex-row justify-end gap-4">
          <Button
            icon={faArrowLeft}
            label="Change Core"
            onClick={() => setPathname('/login/core/select')}
          />
        </div>
      </div>
    </div>
  );
};

export default UserSelectExisting;
