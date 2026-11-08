import { Dialog, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
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
import {
  SettingsCard,
  SettingsPage,
  SettingsRow,
  SettingsSection,
} from 'components/Settings/SettingsBlocks';
import { notifySaved } from 'components/Settings/savedToast';

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
    notifySaved();
  };

  return (
    <>
      <Transition appear show={showChangePassword} as={Fragment}>
        <Dialog
          onClose={() => setShowChangePassword(false)}
          className="relative z-10"
        >
          <ModalBackdrop />
          <div className="fixed inset-0 overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
              <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-4 rounded-xl bg-gray-850 p-6">
                <div className="text-h2 font-extrabold tracking-tight text-gray-300">
                  Change password
                </div>
                <ChangeSelfPasswordForm
                  onSuccess={() => setShowChangePassword(false)}
                  onCancel={() => setShowChangePassword(false)}
                />
              </ModalPanel>
            </div>
          </div>
        </Dialog>
      </Transition>
      <SettingsPage
        title="Profile"
        description={`Profile preferences for ${username}`}
      >
        <SettingsSection title="Account">
          <SettingsCard>
            <InputBox
              label={'Username'}
              value={username}
              type="text"
              onSubmit={rename}
              disabled={userState !== 'logged-in' || !user}
            />
            <SettingsRow label="Password">
              <Button
                label={'Change password'}
                size={'medium'}
                onClick={() => setShowChangePassword(true)}
              />
            </SettingsRow>
          </SettingsCard>
        </SettingsSection>

        <SettingsSection
          title="Appearance"
          lead="Saved in this browser, not on the core."
        >
          <SettingsCard>
            <AppearanceSetting />
            <ToggleBox
              label="Colored console"
              value={coloredConsole}
              onChange={async (value) => setColoredConsole(value)}
              description="Shows the colors and styles servers print in the server console"
            />
          </SettingsCard>
        </SettingsSection>

        <ApiKeysSection enabled={userState === 'logged-in' && !!user} />

        <OwnMinecraftLinks enabled={userState === 'logged-in' && !!user} />
      </SettingsPage>
    </>
  );
};

export default ProfilePage;
