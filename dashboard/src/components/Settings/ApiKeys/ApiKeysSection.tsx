import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { CreatedApiKey } from 'bindings/CreatedApiKey';
import Button from 'components/Atoms/Button';
import { apiKeyKeys, revokeOwnApiKey, useOwnApiKeys } from 'data/ApiKeys';
import { useState } from 'react';
import { toast } from 'react-toastify';
import ApiKeyList from './ApiKeyList';
import CreateApiKeyDialog from './CreateApiKeyDialog';
import CreatedApiKeyDialog from './CreatedApiKeyDialog';

/**
 * The current user's API keys, on the profile page: the list, revoking, and
 * creating a key.
 */
export default function ApiKeysSection({ enabled }: { enabled: boolean }) {
  const { data: keys, isLoading, error } = useOwnApiKeys(enabled);
  const [showCreate, setShowCreate] = useState(false);
  // The new key lives only here, until its dialog has closed.
  const [created, setCreated] = useState<CreatedApiKey | null>(null);
  const [showCreated, setShowCreated] = useState(false);

  return (
    <>
      <CreateApiKeyDialog
        isOpen={showCreate}
        onClose={() => setShowCreate(false)}
        onCreated={(newKey) => {
          setShowCreate(false);
          setCreated(newKey);
          setShowCreated(true);
          toast.success(`Created API key "${newKey.info.name}"`);
        }}
      />
      <CreatedApiKeyDialog
        created={created}
        isOpen={showCreated}
        onClose={() => setShowCreated(false)}
        afterLeave={() => setCreated(null)}
      />
      <div className="mt-8 flex flex-row items-end justify-between gap-4">
        <div>
          <div className="text-h3 font-extrabold text-gray-300">API keys</div>
          <p className="text-medium font-medium tracking-medium text-white/50">
            Keys let the CLI, scripts and agents use Lodestone as you, without
            your password. Revoking a key doesn&apos;t sign you out anywhere
            else.
          </p>
        </div>
        <Button
          label="Create API key"
          icon={faPlus}
          className="shrink-0"
          disabled={!enabled}
          onClick={() => setShowCreate(true)}
        />
      </div>
      <div className="mt-4">
        <ApiKeyList
          keys={keys}
          isLoading={isLoading}
          error={error}
          queryKey={apiKeyKeys.own}
          revoke={(key) => revokeOwnApiKey(key.id)}
          emptyText="You have no API keys."
        />
      </div>
    </>
  );
}
