import { PublicUser } from 'bindings/PublicUser';
import { apiKeyKeys, revokeUserApiKey, useUserApiKeys } from 'data/ApiKeys';
import { useT } from 'i18n';
import ApiKeyList from './ApiKeyList';
import { SettingsSection } from '../SettingsBlocks';

/**
 * Another user's API keys, on the user management page, for owners and
 * admins. Keys can be revoked here, but not created for someone else.
 */
export default function UserApiKeys({ user }: { user: PublicUser }) {
  const t = useT();
  const { data: keys, isLoading, error } = useUserApiKeys(user.uid);

  return (
    <SettingsSection
      title={t('settings.apiKeys.title')}
      lead={t('settings.apiKeys.userLead', { username: user.username })}
    >
      <ApiKeyList
        keys={keys}
        isLoading={isLoading}
        error={error}
        queryKey={apiKeyKeys.ofUser(user.uid)}
        revoke={(key) => revokeUserApiKey(user.uid, key.id)}
        emptyText={t('settings.apiKeys.userEmpty', { username: user.username })}
      />
    </SettingsSection>
  );
}
