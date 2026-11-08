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
import { useState } from 'react';
import { toast } from 'react-toastify';
import { ApiError, formatTimeAgo } from 'utils/util';

const formatDate = (seconds: number) =>
  new Date(seconds * 1000).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });

const formatDateTime = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString();

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
  emptyText: string;
}) {
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
    return (
      <div className="text-medium font-medium tracking-medium text-white/50 italic">
        Loading API keys...
      </div>
    );
  }

  return (
    <>
      <ConfirmDialog
        title="Revoke API key"
        type="danger"
        confirmButtonText="Revoke"
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
          toast.success(`Revoked API key "${toRevoke.name}"`);
          setToRevoke(null);
        }}
      >
        {toRevoke && (
          <>
            Revoke &quot;{toRevoke.name}&quot; ({toRevoke.display})? Anything
            using this key loses access at once. This can&apos;t be undone.
          </>
        )}
      </ConfirmDialog>
      {keys.length === 0 ? (
        <div className="text-medium font-medium tracking-medium text-white/50 italic">
          {emptyText}
        </div>
      ) : (
        <div className="w-full min-w-0 rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
          {keys.map((key) => {
            const expired = isApiKeyExpired(key);
            return (
              <div
                key={key.id}
                className="flex flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3"
              >
                <div className="flex min-w-0 flex-col gap-0.5">
                  <div className="flex min-w-0 flex-row flex-wrap items-baseline gap-x-2">
                    <span className="truncate text-h3 leading-tight font-bold text-gray-300">
                      {key.name}
                    </span>
                    <span className="font-mono text-small text-white/50">
                      {key.display}
                    </span>
                  </div>
                  <div className="text-medium font-medium tracking-medium text-gray-300">
                    {apiKeyScopeSummary(key.scope)}
                  </div>
                  <div className="flex flex-row flex-wrap gap-x-3 text-small font-medium tracking-medium text-white/50">
                    <span title={formatDateTime(key.created_at)}>
                      Created {formatDate(key.created_at)}
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
                        ? 'Expires: Never'
                        : `${expired ? 'Expired' : 'Expires'} ${formatDate(
                            key.expires_at
                          )}`}
                    </span>
                    <span
                      title={
                        key.last_used_at === null
                          ? undefined
                          : formatDateTime(key.last_used_at)
                      }
                    >
                      Last used:{' '}
                      {key.last_used_at === null
                        ? 'Never'
                        : formatTimeAgo(key.last_used_at * 1000)}
                    </span>
                  </div>
                </div>
                <Button
                  label="Revoke"
                  intention="danger"
                  className="shrink-0"
                  onClick={() => setToRevoke(key)}
                />
              </div>
            );
          })}
        </div>
      )}
    </>
  );
}
