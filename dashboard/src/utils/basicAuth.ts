// HTTP basic auth for signing in. This module has no imports, so its tests run
// with plain `node --test`.

/**
 * The Authorization header for basic auth, with the username and password
 * sent as UTF-8 (RFC 7617), which is how the core decodes them. axios's `auth`
 * option sends the username as Latin-1 (and throws on characters beyond it),
 * so a user called José or 用户 could never sign in.
 */
export function basicAuthHeader(username: string, password: string): string {
  const bytes = new TextEncoder().encode(`${username}:${password}`);
  // btoa takes a "binary string": one character per byte
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return `Basic ${btoa(binary)}`;
}
