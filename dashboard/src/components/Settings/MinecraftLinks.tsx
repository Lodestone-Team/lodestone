import { useQueryClient } from '@tanstack/react-query';
import Button from 'components/Atoms/Button';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import {
  unlinkMinecraftAccount,
  useAllMinecraftLinks,
  useOwnMinecraftLinks,
} from 'data/MinecraftLinks';
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

const linkErrorMessage = (error: ApiError | null) =>
  error?.status === 404
    ? 'This core is too old to link Minecraft accounts.'
    : errorToString(error);

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
  const queryClient = useQueryClient();
  const [toRemove, setToRemove] = useState<Link | null>(null);
  if (error) {
    return (
      <SettingsCard>
        <SettingsEmptyRow>{linkErrorMessage(error)}</SettingsEmptyRow>
      </SettingsCard>
    );
  }
  if (!links) return null;
  return (
    <>
      <ConfirmDialog
        title="Remove Minecraft account"
        type="danger"
        confirmButtonText="Remove"
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
          toast.success(`Removed ${toRemove.minecraft_name}`);
          setToRemove(null);
        }}
      >
        {toRemove && (
          <>
            Remove the link to {toRemove.minecraft_name}? It can be linked again
            from Lodestone Launcher.
          </>
        )}
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
              hint={<span className="font-mono">{link.minecraft_uuid}</span>}
            >
              <Button
                label="Remove"
                intention="danger"
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
  const { data, error } = useOwnMinecraftLinks(enabled);
  return (
    <SettingsSection
      title="Minecraft accounts"
      lead="Accounts you linked from Lodestone Launcher, which checks with Microsoft that they are yours. The dashboard shows your name next to them."
    >
      <LinkList
        links={data}
        error={error}
        anyUser={false}
        emptyText="No Minecraft account is linked. Sign in to this core in Lodestone Launcher to link one."
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
  const { data, error } = useAllMinecraftLinks();
  return (
    <SettingsSection
      title="Minecraft accounts"
      lead={`The Minecraft accounts ${username} linked from Lodestone Launcher.`}
    >
      <LinkList
        links={data?.filter((link) => link.uid === uid)}
        error={error}
        anyUser
        emptyText={`${username} has no linked Minecraft accounts.`}
      />
    </SettingsSection>
  );
}
