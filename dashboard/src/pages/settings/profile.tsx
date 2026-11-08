import { Dialog, Transition } from '@headlessui/react';
import { useQueryClient } from '@tanstack/react-query';
import { Fragment, useEffect, useState } from 'react';
import { useDocumentTitle } from 'usehooks-ts';
import { useUserState } from 'components/useUserState';
import { PublicUser } from 'bindings/PublicUser';
import { useUid } from 'data/UserInfo';
import InputBox from 'components/Atoms/Config/InputBox';
import { PageLoading } from 'components/DashboardLayout/LayoutSkeletons';
import { axiosPutSingleValue } from 'utils/util';
import Button from 'components/Atoms/Button';
import ChangeSelfPasswordForm from 'components/Settings/ChangeSelfPasswordForm';
import ApiKeysSection from 'components/Settings/ApiKeys/ApiKeysSection';
import { OwnMinecraftLinks } from 'components/Settings/MinecraftLinks';
import AppearanceSetting from 'components/Settings/AppearanceSetting';
import ToggleBox from 'components/Atoms/Config/ToggleBox';
import { useColoredConsole } from 'utils/hooks';
import { toast } from 'react-toastify';

const ProfilePage = () => {
  useDocumentTitle('User Profile - Lodestone');
  const uid = useUid();
  const queryClient = useQueryClient();
  const { userState, user } = useUserState();
  // read from the user info cache, so a rename shows up everywhere
  const username = userState === 'logged-in' && user ? user.username : 'Guest';
  const [showChangePassword, setShowChangePassword] = useState(false);
  const [coloredConsole, setColoredConsole] = useColoredConsole();

  useEffect(() => {
    if (userState === 'logged-out')
      toast.warn("You're not logged in! Please log in to access this page.", {
        toastId: 'profile-not-logged-in',
      });
  }, [userState]);

  if (userState === 'loading') {
    return <PageLoading className="max-w-2xl" />;
  }

  const rename = async (value: string) => {
    await axiosPutSingleValue(`/user/${uid}/rename`, value);
    queryClient.setQueryData(
      ['user', 'info'],
      (oldInfo: PublicUser | undefined) =>
        oldInfo && { ...oldInfo, username: value }
    );
    queryClient.setQueryData(
      ['user', 'list'],
      (oldList: { [uid: string]: PublicUser } | undefined) =>
        oldList && oldList[uid]
          ? { ...oldList, [uid]: { ...oldList[uid], username: value } }
          : oldList
    );
  };

  return (
    // used to possibly center the content
    <div className="@container relative mx-auto flex h-full w-full max-w-2xl flex-row justify-center">
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
          <div className="fixed inset-0 bg-scrim" />
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

        <div className="mt-8 w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
          <InputBox
            label={'Username'}
            value={username}
            type="text"
            onSubmit={rename}
            disabled={userState !== 'logged-in' || !user}
          />
        </div>

        <div className="mt-8 text-h3 font-extrabold text-gray-300">
          Appearance
        </div>
        {/* saved in this browser, not on the core */}
        <div className="mt-4 w-full min-w-0 overflow-clip rounded-lg border border-gray-faded/30 child:border-b child:border-gray-faded/30 child:last:border-b-0">
          <AppearanceSetting />
          <ToggleBox
            label="Colored console"
            value={coloredConsole}
            onChange={async (value) => setColoredConsole(value)}
            description="Shows the colors and styles servers print in the instance console"
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

        <ApiKeysSection enabled={userState === 'logged-in' && !!user} />

        <OwnMinecraftLinks enabled={userState === 'logged-in' && !!user} />
      </div>
    </div>
  );
};

export default ProfilePage;
