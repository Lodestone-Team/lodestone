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
      <div className="text-medium font-medium tracking-medium text-white/50 italic">
        {linkErrorMessage(error)}
      </div>
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
      {links.length === 0 ? (
        <div className="text-medium font-medium tracking-medium text-white/50 italic">
          {emptyText}
        </div>
      ) : (
        <div className="w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
          {links.map((link) => (
            <div
              key={link.minecraft_uuid}
              className="flex flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3"
            >
              <div className="flex min-w-0 flex-col gap-0.5">
                <span className="truncate text-h3 leading-tight font-bold text-gray-300">
                  {link.minecraft_name}
                </span>
                <span className="font-mono text-small text-white/50">
                  {link.minecraft_uuid}
                </span>
              </div>
              <Button
                label="Remove"
                intention="danger"
                className="shrink-0"
                onClick={() => setToRemove(link)}
              />
            </div>
          ))}
        </div>
      )}
    </>
  );
}

/** The current user's linked Minecraft accounts, on the profile page */
export function OwnMinecraftLinks({ enabled }: { enabled: boolean }) {
  const { data, error } = useOwnMinecraftLinks(enabled);
  return (
    <>
      <div className="mt-8 text-h3 font-extrabold text-gray-300">
        Minecraft accounts
      </div>
      <p className="text-medium font-medium tracking-medium text-white/50">
        Accounts you linked from Lodestone Launcher, which checks with Microsoft
        that they are yours. The dashboard shows your name next to them.
      </p>
      <div className="mt-4">
        <LinkList
          links={data}
          error={error}
          anyUser={false}
          emptyText="No Minecraft account is linked. Sign in to this core in Lodestone Launcher to link one."
        />
      </div>
    </>
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
    <div className="flex flex-col gap-4">
      <div>
        <div className="text-h2 leading-tight font-bold tracking-tight text-gray-300">
          Minecraft Accounts
        </div>
        <div className="text-h3 font-medium tracking-medium text-white/50">
          The Minecraft accounts {username} linked from Lodestone Launcher.
        </div>
      </div>
      <LinkList
        links={data?.filter((link) => link.uid === uid)}
        error={error}
        anyUser
        emptyText={`${username} has no linked Minecraft accounts.`}
      />
    </div>
  );
}
