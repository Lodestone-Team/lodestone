import { useQuery, useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { InstancePublicListing } from 'bindings/InstancePublicListing';
import { PublicListingOverview } from 'bindings/PublicListingOverview';
import { PublicScope } from 'bindings/PublicScope';
import { PublicScopeChange } from 'bindings/PublicScopeChange';
import Checkbox from 'components/Atoms/Checkbox';
import { Toggle } from 'components/Atoms/Toggle';
import { useUserInfo } from 'data/UserInfo';
import { useId, useState } from 'react';
import { toast } from 'react-toastify';
import { axiosWrapper, errorToString } from 'utils/util';
import {
  SettingsCard,
  SettingsEmptyRow,
  SettingsRow,
  SettingsSection,
} from 'components/Settings/SettingsBlocks';
import { notifySaved } from 'components/Settings/savedToast';
import { type I18n, type MessageKey, useI18n, useT } from 'i18n';

// the labels are message keys, translated when rendered
const SCOPE_FLAGS = [
  { key: 'status', labelKey: 'server.publicListing.flag.status' },
  { key: 'address', labelKey: 'server.publicListing.flag.address' },
  { key: 'client_pack', labelKey: 'server.publicListing.flag.clientPack' },
] as const satisfies ReadonlyArray<{
  key: 'status' | 'address' | 'client_pack';
  labelKey: MessageKey;
}>;

/** What anyone sees of a listed instance, as a sentence. */
function shownText({ t, formatList }: I18n, scope: PublicScope) {
  const shown = [
    t('server.publicListing.shown.nameVersion'),
    scope.status && t('server.publicListing.shown.status'),
    scope.address && t('server.publicListing.shown.address'),
    scope.client_pack && t('server.publicListing.shown.clientPack'),
  ].filter((item): item is string => !!item);
  return t('server.publicListing.shownText', { list: formatList(shown) });
}

/**
 * The query keys of the public listing, refreshed after a change so Core
 * Settings and Instance Settings agree.
 */
const overviewKey = ['public_listing'];
const instanceKey = (uuid: string) => ['instances', uuid, 'public_listing'];

/** Changes what an instance shows publicly. Only the owner may. */
async function changeScope(uuid: string, change: PublicScopeChange) {
  return axiosWrapper<PublicScope>({
    method: 'PUT',
    url: `/instance/${uuid}/public_listing`,
    data: JSON.stringify(change),
    headers: { 'Content-Type': 'application/json' },
  });
}

/**
 * One instance's "Public" toggle and what it shows. Editable for the owner,
 * read-only for everyone else.
 */
export function PublicScopeRow({
  uuid,
  label,
  scope,
  switchOn,
  canEdit,
}: {
  uuid: string;
  label: string;
  scope: PublicScope;
  switchOn: boolean;
  canEdit: boolean;
}) {
  const i18n = useI18n();
  const { t } = i18n;
  const queryClient = useQueryClient();
  const [saving, setSaving] = useState(false);
  const labelId = useId();

  const change = async (value: PublicScopeChange) => {
    setSaving(true);
    try {
      const updated = await changeScope(uuid, value);
      queryClient.setQueryData<InstancePublicListing>(
        instanceKey(uuid),
        (old) => old && { ...old, ...updated }
      );
      queryClient.setQueryData<PublicListingOverview>(
        overviewKey,
        (old) =>
          old && {
            ...old,
            instances: old.instances.map((i) =>
              i.uuid === uuid ? { ...i, ...updated } : i
            ),
          }
      );
      notifySaved();
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setSaving(false);
    }
  };

  const description = !canEdit
    ? t('server.publicListing.ownerOnly')
    : !switchOn && !scope.listed
      ? t('server.publicListing.turnOnFirst')
      : scope.listed
        ? switchOn
          ? shownText(i18n, scope)
          : t('server.publicListing.hiddenWhileOff', {
              shown: shownText(i18n, scope),
            })
        : t('server.publicListing.notShown');

  return (
    <SettingsRow
      className="group relative"
      label={<span className="block truncate">{label}</span>}
      labelId={labelId}
      hint={description}
      below={
        <div className="flex flex-row flex-wrap gap-x-6 gap-y-2">
          {SCOPE_FLAGS.map(({ key, labelKey }) => (
            <Checkbox
              key={key}
              label={t(labelKey)}
              checked={scope[key]}
              disabled={!canEdit || saving}
              onChange={(value) => void change({ [key]: value })}
            />
          ))}
        </div>
      }
    >
      <span className="text-small font-medium whitespace-nowrap text-gray-500">
        {t('server.publicListing.public')}
      </span>
      <Toggle
        value={scope.listed}
        onChange={(value) => void change({ listed: value })}
        // unlisting always works; listing needs the core setting
        disabled={!canEdit || saving || (!switchOn && !scope.listed)}
        aria-labelledby={labelId}
      />
    </SettingsRow>
  );
}

/**
 * The instances in Core Settings → Public instances, for the owner. Renders
 * nothing on cores without per-instance scopes.
 */
export function PublicInstanceRows({ switchOn }: { switchOn: boolean }) {
  const t = useT();
  const { data: overview } = useQuery({
    queryKey: overviewKey,
    queryFn: () =>
      axiosWrapper<PublicListingOverview>({
        method: 'GET',
        url: '/public_listing',
      }),
    retry: false,
  });
  if (!overview) return null;
  if (overview.instances.length === 0)
    return (
      <SettingsEmptyRow>{t('server.publicListing.noServers')}</SettingsEmptyRow>
    );
  return (
    <>
      {overview.instances.map((instance) => (
        <PublicScopeRow
          key={instance.uuid}
          uuid={instance.uuid}
          label={instance.name}
          scope={instance}
          switchOn={switchOn}
          canEdit
        />
      ))}
    </>
  );
}

/**
 * Instance Settings → Sharing: whether the instance is public and what it
 * shows, while the owner's "List instances publicly" core setting is on.
 * Lodestone Launcher reads that list to join the server. Renders nothing on
 * cores without it.
 */
export function PublicListingSetting({ instance }: { instance: InstanceInfo }) {
  const t = useT();
  const { data: userInfo } = useUserInfo();
  const { data: listing } = useQuery({
    queryKey: instanceKey(instance.uuid),
    queryFn: () =>
      axiosWrapper<InstancePublicListing>({
        method: 'GET',
        url: `/instance/${instance.uuid}/public_listing`,
      }),
    enabled: instance.game_type.type === 'MinecraftJava',
    retry: false,
  });
  // cores before scopes have no flags beyond `listed`
  if (!listing || typeof listing.status !== 'boolean') return null;

  return (
    <SettingsSection
      id="server-settings-sharing"
      title={t('server.publicListing.title')}
      lead={t('server.publicListing.lead')}
    >
      <SettingsCard>
        <PublicScopeRow
          uuid={instance.uuid}
          label={t('server.publicListing.listedPublicly')}
          scope={listing}
          switchOn={listing.list_instances_publicly}
          canEdit={userInfo?.is_owner ?? false}
        />
      </SettingsCard>
    </SettingsSection>
  );
}
