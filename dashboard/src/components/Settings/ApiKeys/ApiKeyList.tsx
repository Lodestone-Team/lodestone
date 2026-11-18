import { useQueryClient } from '@tanstack/react-query';
import { ApiKeyInfo } from 'bindings/ApiKeyInfo';
import Button from 'components/Atoms/Button';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import WarningAlert from 'components/Atoms/WarningAlert';
import {
  apiKeyErrorMessage,
  apiKeyScopeCounts,
  apiKeyScopeSummary,
  isApiKeyExpired,
  removeApiKeyFromCache,
} from 'data/ApiKeys';
import { useI18n } from 'i18n';
import Label from 'components/Atoms/Label';
import {
  KindIcon,
  MonoText,
  MonoValue,
  Num,
  ObjectChip,
  StatusBadge,
} from 'components/Atoms/Values';
import {
  Fact,
  FactLine,
  factsTitle,
  ObjectCard,
  ObjectToast,
} from 'components/Values';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { ApiError, formatTimeAgo } from 'utils/util';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import {
  SettingsCard,
  SettingsEmptyRow,
  SettingsRow,
} from 'components/Settings/SettingsBlocks';

/**
 * A list of API keys, each with a Revoke button that confirms first.
 */
export default function ApiKeyList({
  keys,
  isLoading,
  error,
  queryKey,
  revoke,
  emptyText,
}: {
  keys: ApiKeyInfo[] | undefined;
  isLoading: boolean;
  error: ApiError | null;
  /** The query the keys come from; a revoked key is removed from it */
  queryKey: string[];
  revoke: (key: ApiKeyInfo) => Promise<unknown>;
  /** shown when there are no keys (translated) */
  emptyText: string;
}) {
  const {
    t,
    formatDate: formatDateIn,
    formatDateTime: formatDateTimeIn,
  } = useI18n();
  const formatDate = (seconds: number) =>
    formatDateIn(seconds * 1000, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  const formatDateTime = (seconds: number) => formatDateTimeIn(seconds * 1000);
  const queryClient = useQueryClient();
  const [toRevoke, setToRevoke] = useState<ApiKeyInfo | null>(null);

  if (error) {
    return (
      <WarningAlert>
        <p>{apiKeyErrorMessage(error)}</p>
      </WarningAlert>
    );
  }
  if (isLoading || !keys) {
    // key rows: name and prefix, scopes, dates, and the revoke button
    return (
      <SkeletonGroup
        label={t('settings.apiKeys.loading')}
        className="w-full min-w-0 divide-y divide-fade-700 rounded-xl border border-fade-700 bg-gray-850"
      >
        {[0, 1].map((i) => (
          <div
            key={i}
            className="flex flex-row items-center justify-between gap-4 px-4 py-3"
          >
            <div className="flex grow flex-col gap-2">
              <Skeleton className="h-4 w-48 rounded-sm" />
              <Skeleton className="h-3.5 w-64 max-w-full rounded-sm" />
              <Skeleton className="h-3 w-56 max-w-full rounded-sm" />
            </div>
            <Skeleton className="h-7 w-20 shrink-0" />
          </div>
        ))}
      </SkeletonGroup>
    );
  }

  return (
    <>
      <ConfirmDialog
        title={t('settings.apiKeys.revokeTitle')}
        type="danger"
        confirmButtonText={t('settings.apiKeys.revoke')}
        closeButtonText={t('common.cancel')}
        isOpen={toRevoke !== null}
        onClose={() => setToRevoke(null)}
        onConfirm={async () => {
          if (!toRevoke) return;
          try {
            await revoke(toRevoke);
          } catch (e) {
            toast.error(apiKeyErrorMessage(e));
            return;
          }
          removeApiKeyFromCache(queryClient, queryKey, toRevoke.id);
          toast.success(
            <ObjectToast
              object={<ObjectChip kind="apiKey" name={toRevoke.name} />}
              outcome={t('settings.apiKeys.toast.revoked')}
            />
          );
          setToRevoke(null);
        }}
        object={
          toRevoke && (
            <ObjectCard
              icon={<KindIcon kind="apiKey" className="w-5" />}
              name={toRevoke.name}
              facts={
                <FactLine>
                  <Fact label={t('values.key.prefix')}>
                    <MonoText text={toRevoke.display} />
                  </Fact>
                  <Fact label={t('values.key.created')}>
                    {formatDate(toRevoke.created_at)}
                  </Fact>
                </FactLine>
              }
            />
          )
        }
      >
        {t('settings.apiKeys.revokeBody')}
      </ConfirmDialog>
      <SettingsCard>
        {keys.length === 0 ? (
          <SettingsEmptyRow>{emptyText}</SettingsEmptyRow>
        ) : (
          keys.map((key) => {
            const expired = isApiKeyExpired(key);
            const counts = apiKeyScopeCounts(key.scope);
            const never = t('settings.apiKeys.expiry.never');
            const created = formatDate(key.created_at);
            const expires =
              key.expires_at === null ? never : formatDate(key.expires_at);
            const lastUsed =
              key.last_used_at === null
                ? never
                : formatTimeAgo(key.last_used_at * 1000);
            return (
              <SettingsRow
                key={key.id}
                label={
                  <span className="flex min-w-0 flex-row flex-wrap items-center gap-x-2">
                    <span className="truncate">{key.name}</span>
                    <MonoValue>{key.display}</MonoValue>
                  </span>
                }
                hint={
                  <span className="flex flex-col gap-1">
                    <span className="flex flex-row flex-wrap items-center gap-x-3 gap-y-1">
                      <Label size="small" color="gray" className="w-fit">
                        {apiKeyScopeSummary(key.scope)}
                      </Label>
                      {counts && (
                        <>
                          <Fact label={t('values.key.servers')}>
                            <Num value={counts.instances} />
                          </Fact>
                          <Fact label={t('values.key.permissions')}>
                            <Num value={counts.permissions} />
                          </Fact>
                        </>
                      )}
                    </span>
                    <FactLine
                      title={factsTitle([
                        { label: t('values.key.created'), value: created },
                        { label: t('values.key.expires'), value: expires },
                        { label: t('values.key.lastUsed'), value: lastUsed },
                      ])}
                    >
                      <Fact
                        label={t('values.key.created')}
                        title={formatDateTime(key.created_at)}
                      >
                        {created}
                      </Fact>
                      <Fact
                        label={t('values.key.expires')}
                        title={
                          key.expires_at === null
                            ? undefined
                            : formatDateTime(key.expires_at)
                        }
                      >
                        {expired ? (
                          <span className="inline-flex items-center gap-1.5">
                            <StatusBadge tone="danger">
                              {t('settings.apiKeys.expired')}
                            </StatusBadge>
                            {expires}
                          </span>
                        ) : (
                          expires
                        )}
                      </Fact>
                      <Fact
                        label={t('values.key.lastUsed')}
                        title={
                          key.last_used_at === null
                            ? undefined
                            : formatDateTime(key.last_used_at)
                        }
                      >
                        <span className="text-ink-muted">{lastUsed}</span>
                      </Fact>
                    </FactLine>
                  </span>
                }
              >
                <Button
                  label={t('settings.apiKeys.revoke')}
                  intention="danger"
                  onClick={() => setToRevoke(key)}
                />
              </SettingsRow>
            );
          })
        )}
      </SettingsCard>
    </>
  );
}
