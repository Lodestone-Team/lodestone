import { useQueryClient } from '@tanstack/react-query';
import { ApiKeyInfo } from 'bindings/ApiKeyInfo';
import clsx from 'clsx';
import Button from 'components/Atoms/Button';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import WarningAlert from 'components/Atoms/WarningAlert';
import {
  apiKeyErrorMessage,
  apiKeyScopeSummary,
  isApiKeyExpired,
  removeApiKeyFromCache,
} from 'data/ApiKeys';
import { useI18n } from 'i18n';
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
          toast.success(t('settings.apiKeys.revoked', { name: toRevoke.name }));
          setToRevoke(null);
        }}
      >
        {toRevoke &&
          t('settings.apiKeys.revokeBody', {
            name: toRevoke.name,
            display: toRevoke.display,
          })}
      </ConfirmDialog>
      <SettingsCard>
        {keys.length === 0 ? (
          <SettingsEmptyRow>{emptyText}</SettingsEmptyRow>
        ) : (
          keys.map((key) => {
            const expired = isApiKeyExpired(key);
            return (
              <SettingsRow
                key={key.id}
                label={
                  <span className="flex min-w-0 flex-row flex-wrap items-baseline gap-x-2">
                    <span className="truncate">{key.name}</span>
                    <span className="font-mono text-small font-medium text-gray-500">
                      {key.display}
                    </span>
                  </span>
                }
                hint={
                  <>
                    <span className="block text-gray-400">
                      {apiKeyScopeSummary(key.scope)}
                    </span>
                    <span className="flex flex-row flex-wrap gap-x-3">
                      <span title={formatDateTime(key.created_at)}>
                        {t('settings.apiKeys.createdOn', {
                          date: formatDate(key.created_at),
                        })}
                      </span>
                      <span
                        className={clsx(expired && 'text-red-200')}
                        title={
                          key.expires_at === null
                            ? undefined
                            : formatDateTime(key.expires_at)
                        }
                      >
                        {key.expires_at === null
                          ? t('settings.apiKeys.expiresNever')
                          : t(
                              expired
                                ? 'settings.apiKeys.expiredOn'
                                : 'settings.apiKeys.expiresOn',
                              { date: formatDate(key.expires_at) }
                            )}
                      </span>
                      <span
                        title={
                          key.last_used_at === null
                            ? undefined
                            : formatDateTime(key.last_used_at)
                        }
                      >
                        {key.last_used_at === null
                          ? t('settings.apiKeys.lastUsedNever')
                          : t('settings.apiKeys.lastUsed', {
                              time: formatTimeAgo(key.last_used_at * 1000),
                            })}
                      </span>
                    </span>
                  </>
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
