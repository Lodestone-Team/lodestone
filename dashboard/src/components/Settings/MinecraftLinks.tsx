import { useQueryClient } from '@tanstack/react-query';
import Button from 'components/Atoms/Button';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import {
  unlinkMinecraftAccount,
  useAllMinecraftLinks,
  useOwnMinecraftLinks,
} from 'data/MinecraftLinks';
import { TFunction, useT } from 'i18n';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { ApiError, errorToString } from 'utils/util';
import {
  SettingsCard,
  SettingsEmptyRow,
  SettingsRow,
  SettingsSection,
} from './SettingsBlocks';

type Link = { minecraft_uuid: string; minecraft_name: string; uid: string };

const linkErrorMessage = (t: TFunction, error: ApiError | null) =>
  error?.status === 404 ? t('settings.minecraft.tooOld') : errorToString(error);

/** A list of linked accounts, each with a Remove button that confirms first */
function LinkList({
  links,
  error,
  emptyText,
  anyUser,
}: {
  links: Link[] | undefined;
  error: ApiError | null;
  emptyText: string;
  /** Remove through the owner's route, which removes anyone's links */
  anyUser: boolean;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const [toRemove, setToRemove] = useState<Link | null>(null);
  if (error) {
    return (
      <SettingsCard>
        <SettingsEmptyRow>{linkErrorMessage(t, error)}</SettingsEmptyRow>
      </SettingsCard>
    );
  }
  if (!links) return null;
  return (
    <>
      <ConfirmDialog
        title={t('settings.minecraft.removeTitle')}
        type="danger"
        confirmButtonText={t('common.remove')}
        closeButtonText={t('common.cancel')}
        isOpen={toRemove !== null}
        onClose={() => setToRemove(null)}
        onConfirm={async () => {
          if (!toRemove) return;
          try {
            await unlinkMinecraftAccount(toRemove.minecraft_uuid, anyUser);
          } catch (e) {
            toast.error(errorToString(e));
            return;
          }
          await queryClient.invalidateQueries({
            queryKey: ['minecraft_links'],
          });
          toast.success(
            t('settings.minecraft.removed', { name: toRemove.minecraft_name })
          );
          setToRemove(null);
        }}
      >
        {toRemove &&
          t('settings.minecraft.removeBody', {
            name: toRemove.minecraft_name,
          })}
      </ConfirmDialog>
      <SettingsCard>
        {links.length === 0 ? (
          <SettingsEmptyRow>{emptyText}</SettingsEmptyRow>
        ) : (
          links.map((link) => (
            <SettingsRow
              key={link.minecraft_uuid}
              label={
                <span className="block truncate">{link.minecraft_name}</span>
              }
              hint={
                <span className="font-mono break-all">
                  {link.minecraft_uuid}
                </span>
              }
            >
              <Button
                label={t('common.remove')}
                intention="danger"
                className="whitespace-nowrap"
                onClick={() => setToRemove(link)}
              />
            </SettingsRow>
          ))
        )}
      </SettingsCard>
    </>
  );
}

/** The current user's linked Minecraft accounts, on the profile page */
export function OwnMinecraftLinks({ enabled }: { enabled: boolean }) {
  const t = useT();
  const { data, error } = useOwnMinecraftLinks(enabled);
  return (
    <SettingsSection
      title={t('settings.minecraft.title')}
      lead={t('settings.minecraft.ownLead')}
    >
      <LinkList
        links={data}
        error={error}
        anyUser={false}
        emptyText={t('settings.minecraft.ownEmpty')}
      />
    </SettingsSection>
  );
}

/** Another user's linked Minecraft accounts, for the owner */
export function UserMinecraftLinks({
  uid,
  username,
}: {
  uid: string;
  username: string;
}) {
  const t = useT();
  const { data, error } = useAllMinecraftLinks();
  return (
    <SettingsSection
      title={t('settings.minecraft.title')}
      lead={t('settings.minecraft.userLead', { username })}
    >
      <LinkList
        links={data?.filter((link) => link.uid === uid)}
        error={error}
        anyUser
        emptyText={t('settings.minecraft.userEmpty', { username })}
      />
    </SettingsSection>
  );
}
