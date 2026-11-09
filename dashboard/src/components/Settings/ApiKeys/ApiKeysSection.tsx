import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { CreatedApiKey } from 'bindings/CreatedApiKey';
import Button from 'components/Atoms/Button';
import { apiKeyKeys, revokeOwnApiKey, useOwnApiKeys } from 'data/ApiKeys';
import { useT } from 'i18n';
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
  const t = useT();
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
          toast.success(
            t('settings.apiKeys.created', { name: newKey.info.name })
          );
        }}
      />
      <CreatedApiKeyDialog
        created={created}
        isOpen={showCreated}
        onClose={() => setShowCreated(false)}
        afterLeave={() => setCreated(null)}
      />
      <SettingsSection
        title={t('settings.apiKeys.title')}
        lead={t('settings.apiKeys.ownLead')}
        action={
          <Button
            label={t('settings.apiKeys.create')}
            icon={faPlus}
            className="whitespace-nowrap"
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
          emptyText={t('settings.apiKeys.ownEmpty')}
        />
      </SettingsSection>
    </>
  );
}
