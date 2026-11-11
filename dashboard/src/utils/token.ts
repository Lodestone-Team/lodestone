import { jwtDecode } from 'jwt-decode';

/** Why a session token can't be used, as a key of the `shell.token.*` messages. */
export type TokenProblem = 'invalid' | 'invalidExp' | 'invalidUid';

/**
 * The claims of a session token, or why it can't be used. Not verified: the core
 * checks the token on every request.
 *
 * A token from signing in with a password is a JWT with `uid` and `exp`. An API key
 * (`lsk_…`) is no JWT: it acts as `keyUid`, the user the core said it signs in as
 * (`POST /user/login_api_key`), and doesn't expire. The core answers 401 once it is
 * revoked, which logs out.
 */
export function tokenClaims(
  token: string,
  keyUid?: string
): { uid: string; exp: number } | TokenProblem {
  if (token.startsWith('lsk_')) {
    return keyUid ? { uid: keyUid, exp: Infinity } : 'invalidUid';
  }
  let payload: { exp?: unknown; uid?: unknown };
  try {
    payload = jwtDecode(token);
  } catch {
    return 'invalid';
  }
  if (typeof payload !== 'object' || payload === null) return 'invalid';
  if (typeof payload.exp !== 'number') return 'invalidExp';
  if (typeof payload.uid !== 'string') return 'invalidUid';
  return { uid: payload.uid, exp: payload.exp };
}
