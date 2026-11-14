// Where the instance view (InstanceViewLayout) sends the user when the
// selected instance or the instance list changes. Its tests run with plain
// `node --test`.
import { BASE_PATH, stripBasePath } from '../../utils/basePath.ts';

/**
 * The page to go to, or null to stay. `windowPathname` is the browser's path
 * (window.location.pathname), which includes the base path the dashboard is
 * served under: it is taken off first, as react-router does. Reading the raw
 * path under /host/app never matched /dashboard, so every instance update
 * jumped to Overview (and closed the settings modal). `basePath` is only
 * passed by tests.
 */
export function instanceRedirect(
  windowPathname: string,
  instanceSelected: boolean,
  basePath = BASE_PATH
): string | null {
  const pathname = stripBasePath(windowPathname, basePath);
  // the settings modal is open over this page: its URL is a settings page
  if (pathname.startsWith('/settings/')) return null;
  const onInstanceTab = pathname.startsWith('/dashboard');
  if (instanceSelected) return onInstanceTab ? null : '/dashboard/overview';
  return onInstanceTab ? '/' : null;
}
