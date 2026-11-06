import { LodestoneContext } from 'data/LodestoneContext';
import { useUserInfo } from 'data/UserInfo';
import { useContext } from 'react';
import type { UserState } from './UserMenu';

/**
 * Whether the current user is logged in, derived from the token and the
 * user info query (no effect, so it is never a render behind).
 */
export function useUserState() {
  const { token } = useContext(LodestoneContext);
  const { isPending, isError, data: user } = useUserInfo();
  let userState: UserState;
  if (!token) userState = 'logged-out';
  else if (isPending) userState = 'loading';
  else if (isError) userState = 'logged-out';
  else userState = 'logged-in';
  return { userState, user };
}
