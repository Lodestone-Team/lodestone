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
import LanguageSetting from 'components/Settings/LanguageSetting';
import ToggleBox from 'components/Atoms/Config/ToggleBox';
import { useColoredConsole } from 'utils/hooks';
import { toast } from 'react-toastify';
import { useT } from 'i18n';
import {
  SettingsCard,
  SettingsPage,
  SettingsRow,
  SettingsSection,
} from 'components/Settings/SettingsBlocks';
import { notifySaved } from 'components/Settings/savedToast';

const ProfilePage = () => {
  const t = useT();
  useDocumentTitle(
    t('common.pageTitle', { page: t('settings.profile.documentTitle') })
  );
  const uid = useUid();
  const queryClient = useQueryClient();
  const { userState, user } = useUserState();
  // read from the user info cache, so a rename shows up everywhere
  const username =
    userState === 'logged-in' && user
      ? user.username
      : t('settings.profile.guest');
  const [showChangePassword, setShowChangePassword] = useState(false);
  const [coloredConsole, setColoredConsole] = useColoredConsole();

  useEffect(() => {
    if (userState === 'logged-out')
      toast.warn(t('settings.profile.notLoggedIn'), {
        toastId: 'profile-not-logged-in',
      });
  }, [userState, t]);

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
          className="relative z-60"
        >
          <ModalBackdrop />
          <div className="fixed inset-0 overflow-y-auto">
            <div className="flex min-h-full items-center justify-center p-4">
              <ModalPanel className="flex w-[500px] flex-col items-stretch justify-center gap-4 rounded-xl bg-gray-850 p-6">
                <div className="text-h2 font-extrabold tracking-tight text-gray-300">
                  {t('settings.profile.changePassword')}
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
      <SettingsPage title={t('settings.profile.title')}>
        <SettingsSection title={t('settings.profile.account')}>
          <SettingsCard>
            <InputBox
              label={t('settings.profile.username')}
              value={username}
              type="text"
              onSubmit={rename}
              disabled={userState !== 'logged-in' || !user}
            />
            <SettingsRow label={t('settings.profile.password')}>
              <Button
                label={t('settings.profile.changePassword')}
                size={'medium'}
                className="whitespace-nowrap"
                onClick={() => setShowChangePassword(true)}
              />
            </SettingsRow>
          </SettingsCard>
        </SettingsSection>

        <SettingsSection
          title={t('settings.profile.appearance')}
          lead={t('settings.profile.appearanceLead')}
        >
          <SettingsCard>
            <LanguageSetting />
            <AppearanceSetting />
            <ToggleBox
              label={t('settings.profile.coloredConsole')}
              value={coloredConsole}
              onChange={async (value) => setColoredConsole(value)}
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
