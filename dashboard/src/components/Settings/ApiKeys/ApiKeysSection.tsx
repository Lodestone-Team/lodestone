import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { CreatedApiKey } from 'bindings/CreatedApiKey';
import Button from 'components/Atoms/Button';
import { apiKeyKeys, revokeOwnApiKey, useOwnApiKeys } from 'data/ApiKeys';
import { useState } from 'react';
import { toast } from 'react-toastify';
import ApiKeyList from './ApiKeyList';
import CreateApiKeyDialog from './CreateApiKeyDialog';
import CreatedApiKeyDialog from './CreatedApiKeyDialog';
import { SettingsSection } from '../SettingsBlocks';

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
      <SettingsSection
        title="API keys"
        lead={
          <>
            Keys let the CLI, scripts and agents use Lodestone as you, without
            your password. Revoking a key doesn&apos;t sign you out anywhere
            else.
          </>
        }
        action={
          <Button
            label="Create API key"
            icon={faPlus}
            disabled={!enabled}
            onClick={() => setShowCreate(true)}
          />
        }
      >
        <ApiKeyList
          keys={keys}
          isLoading={isLoading}
          error={error}
          queryKey={apiKeyKeys.own}
          revoke={(key) => revokeOwnApiKey(key.id)}
          emptyText="You have no API keys."
        />
      </SettingsSection>
    </>
  );
}
