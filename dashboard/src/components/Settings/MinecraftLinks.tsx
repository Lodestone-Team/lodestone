import { useQueryClient } from '@tanstack/react-query';
import Button from 'components/Atoms/Button';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import {
  unlinkMinecraftAccount,
  useAllMinecraftLinks,
  useOwnMinecraftLinks,
} from 'data/MinecraftLinks';
import { TFunction, useT } from 'i18n';
import { KindIcon, MonoText, ObjectChip } from 'components/Atoms/Values';
import { FactLine, ObjectCard, ObjectToast } from 'components/Values';
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
        object={
          toRemove && (
            <ObjectCard
              icon={<KindIcon kind="account" className="w-5" />}
              name={toRemove.minecraft_name}
              facts={
                <FactLine>
                  <MonoText text={toRemove.minecraft_uuid} max={40} />
                </FactLine>
              }
            />
          )
        }
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
            <ObjectToast
              object={
                <ObjectChip kind="account" name={toRemove.minecraft_name} />
              }
              outcome={t('settings.minecraft.toast.removed')}
            />
          );
          setToRemove(null);
        }}
      >
        {t('settings.minecraft.removeBody')}
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
              hint={<MonoText text={link.minecraft_uuid} max={40} />}
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
export function UserMinecraftLinks({ uid }: { uid: string }) {
  const t = useT();
  const { data, error } = useAllMinecraftLinks();
  return (
    <SettingsSection
      title={t('settings.minecraft.title')}
      lead={t('settings.minecraft.userLead')}
    >
      <LinkList
        links={data?.filter((link) => link.uid === uid)}
        error={error}
        anyUser
        emptyText={t('settings.minecraft.userEmpty')}
      />
    </SettingsSection>
  );
}
