import { Dialog, Transition } from '@headlessui/react';
import { Fragment, useContext, useEffect, useState } from 'react';
import { useDocumentTitle } from 'usehooks-ts';
import { UserState } from 'components/UserMenu';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { LodestoneContext } from 'data/LodestoneContext';
import { useUid, useUserInfo } from 'data/UserInfo';
import { useLocation } from 'react-router-dom';
import InputBox from 'components/Atoms/Config/InputBox';
import Spinner from 'components/DashboardLayout/Spinner';
import { axiosPutSingleValue } from 'utils/util';
import Button from 'components/Atoms/Button';
import ChangeSelfPasswordForm from 'components/Settings/ChangeSelfPasswordForm';
import { toast } from 'react-toastify';

const ProfilePage = () => {
  useDocumentTitle('Profile - Lodestone');
  const { token } = useContext(LodestoneContext);
  const uid = useUid();
  const { isLoading, isError, data: user } = useUserInfo();
  const [userState, setUserState] = useState<UserState>('logged-out');

  const [username, setUsername] = useState(
    userState === 'logged-in' && user ? `${user.username}` : 'Guest'
  );
  const [showChangePassword, setShowChangePassword] = useState(false);
  const [stopLoading, setStopLoading] = useState(false);

  useEffect(() => {
    if (!token) {
      setUserState('logged-out');
    } else if (isLoading) {
      setUserState('loading');
      return;
    } else if (isError) {
      setUserState('logged-out');
      return;
    } else {
      setUserState('logged-in');
      setUsername(user.username);
    }
  }, [token, isLoading, isError, user]);

  useEffect(() => {
    //give time for user to load
    setTimeout(() => {
      setStopLoading(true);
    }, 1000);
  }, []);

  if (stopLoading && userState !== 'logged-in')
    toast.warn("You're not logged in! Please log in to access this page.");

  if (isLoading && !stopLoading) {
    return <Spinner />;
  }

  return (
    // used to possibly center the content
    <div className="relative mx-auto flex h-full w-full max-w-2xl flex-row justify-center @container">
      <Transition
        appear
        show={showChangePassword}
        as={Fragment}
        enter="ease-out duration-200"
        enterFrom="opacity-0"
        enterTo="opacity-100"
        leave="ease-in duration-150"
        leaveFrom="opacity-100"
        leaveTo="opacity-0"
      >
        <Dialog
          onClose={() => setShowChangePassword(false)}
          className="relative z-10"
        >
          <div className="fixed inset-0 bg-gray-900/60" />
          <div className="fixed inset-0 overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
              <Dialog.Panel className="flex w-[500px] flex-col items-stretch justify-center gap-4 rounded-xl bg-gray-850 p-6">
                <div className="text-h2 font-extrabold tracking-tight text-gray-300">
                  Change Password
                </div>
                <ChangeSelfPasswordForm
                  onSuccess={() => setShowChangePassword(false)}
                  onCancel={() => setShowChangePassword(false)}
                />
              </Dialog.Panel>
            </div>
          </div>
        </Dialog>
      </Transition>
      <div className="flex w-full grow flex-col items-stretch gap-2 px-4 pt-8">
        <div>
          <h1 className="dashboard-instance-heading text-gray-300">Profile</h1>
          <p className="mt-1 text-medium font-mediumbold">
            Profile preferences for {username}
          </p>
        </div>

        <div className="mt-8 w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
          <InputBox
            label={'Username'}
            value={username}
            type="text"
            isLoading={isLoading}
            onSubmit={async (value) => {
              await axiosPutSingleValue(`/user/${uid}/rename`, value);
              setUsername(value);
            }}
            disabled={userState !== 'logged-in' || !user}
          />
        </div>

        <div className="mt-8 text-h3 font-extrabold text-gray-300">
          Password and Authentication
        </div>
        <div className="mt-4">
          <Button
            label={'Change Password'}
            size={'medium'}
            onClick={() => setShowChangePassword(true)}
          />
        </div>
      </div>
    </div>
  );
};

export default ProfilePage;
