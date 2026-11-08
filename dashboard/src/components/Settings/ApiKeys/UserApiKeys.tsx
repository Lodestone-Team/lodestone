import { PublicUser } from 'bindings/PublicUser';
import { apiKeyKeys, revokeUserApiKey, useUserApiKeys } from 'data/ApiKeys';
import ApiKeyList from './ApiKeyList';

/**
 * Another user's API keys, on the user management page, for owners and
 * admins. Keys can be revoked here, but not created for someone else.
 */
export default function UserApiKeys({ user }: { user: PublicUser }) {
  const { data: keys, isLoading, error } = useUserApiKeys(user.uid);

  return (
    <div className="flex flex-col gap-4">
      <div>
        <div className="text-h2 leading-tight font-bold tracking-tight text-gray-300">
          API Keys
        </div>
        <div className="text-h3 font-medium tracking-medium text-white/50">
          The keys {user.username} created. Revoke a key to cut off what uses it
          at once. Only {user.username} can create keys for themselves.
        </div>
      </div>
      <ApiKeyList
        keys={keys}
        isLoading={isLoading}
        error={error}
        queryKey={apiKeyKeys.ofUser(user.uid)}
        revoke={(key) => revokeUserApiKey(user.uid, key.id)}
        emptyText={`${user.username} has no API keys.`}
      />
    </div>
  );
}
