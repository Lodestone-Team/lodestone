import UserMenu from 'components/UserMenu';
import CoreConnectionStatus from './CoreConnectionStatus';

/**
 * The bottom of a sidebar: the connected core and its status, and under it the
 * account menu and (optionally) the settings button.
 */
export default function SidebarFooter({
  showSettings = true,
}: {
  showSettings?: boolean;
}) {
  return (
    <div className="sticky bottom-0 flex w-full flex-col gap-1 border-t border-fade-700 bg-gray-850 py-2">
      <CoreConnectionStatus />
      <UserMenu showSettings={showSettings} />
    </div>
  );
}
