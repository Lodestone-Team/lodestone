import { useContext } from 'react';
import {
  faServer,
  faUser,
  IconDefinition,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { RadioGroup } from '@headlessui/react';
import { SettingsContext } from 'data/SettingsContext';
import Avatar from 'components/Atoms/Avatar';
import clsx from 'clsx';
import { useUid } from 'data/UserInfo';
import { AccountSettingsTabList, CoreSettingsTabList } from '../../../pages';
import { settingsNavItemClass } from 'components/Settings/SettingsToc';
import { SettingsLink, SettingsTag } from 'components/Settings/SettingsBlocks';

// the highlight of the current page, on the gray-875 sidebar
const ACTIVE_CLASS =
  'bg-gray-800 text-gray-300 light:outline-1 light:outline-fade-700 light:outline-solid';

const pageGroups = [
  { label: 'Core', icon: faServer, pages: CoreSettingsTabList },
  { label: 'Account', icon: faUser, pages: AccountSettingsTabList },
];

/** A caption over a group of the list, e.g. "Core" */
function GroupLabel({
  icon,
  children,
}: {
  icon?: IconDefinition;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-row items-center gap-1.5 px-2.5 pb-1 text-small font-mediumbold text-gray-550 light:text-ink-muted">
      {icon && <FontAwesomeIcon icon={icon} className="w-3" />}
      {children}
    </div>
  );
}

/** The settings sidebar: the pages in captioned groups, then the users */
export default function SettingsLeftNav({
  current,
  className,
}: {
  /** the page shown, e.g. "general" */
  current?: string;
  className?: string;
}) {
  const { setPathname } = useContext(BrowserLocationContext);
  const { userList, selectedUser, selectUser } = useContext(SettingsContext);
  const uid = useUid();
  const users = Object.values(userList);

  return (
    <div
      className={clsx(
        'flex h-full flex-col gap-6 overflow-y-auto px-3 pt-6 pb-4',
        className
      )}
    >
      <nav aria-label="Settings" className="flex w-full flex-col gap-4">
        {pageGroups.map((group) => (
          <div key={group.label} className="flex flex-col gap-0.5">
            <GroupLabel icon={group.icon}>{group.label}</GroupLabel>
            {group.pages.map((setting) => (
              <button
                key={setting.path}
                type="button"
                aria-current={current === setting.path ? 'page' : undefined}
                className={settingsNavItemClass({
                  active: current === setting.path,
                  activeClass: ACTIVE_CLASS,
                })}
                onClick={() => setPathname(`/settings/${setting.path}`)}
              >
                {setting.title}
              </button>
            ))}
          </div>
        ))}
      </nav>

      {users.length > 0 && (
        <div className="flex min-h-0 w-full flex-col gap-0.5">
          <div className="flex flex-row items-center justify-between pr-2.5">
            <GroupLabel>Users</GroupLabel>
            <SettingsLink
              muted
              className="pb-1 text-small"
              disabled={!selectedUser}
              onClick={() => {
                selectUser(null);
              }}
            >
              Unselect user
            </SettingsLink>
          </div>
          <RadioGroup
            value={selectedUser}
            onChange={selectUser}
            aria-label="Users"
            className="flex min-h-0 w-full flex-col items-stretch gap-0.5"
          >
            {users
              .sort((a, b) =>
                a.uid === uid
                  ? -1
                  : b.uid === uid
                    ? 1
                    : a.username.localeCompare(b.username)
              )
              .map((user) => (
                <RadioGroup.Option
                  key={user.uid}
                  value={user}
                  className="rounded-lg focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden"
                >
                  {({ checked }) => (
                    <div
                      className={clsx(
                        settingsNavItemClass({
                          active: checked,
                          activeClass: ACTIVE_CLASS,
                        }),
                        'cursor-pointer'
                      )}
                      onClick={() => setPathname(`/settings/users`)}
                    >
                      <Avatar name={user.uid} />
                      <span className="truncate">{user.username}</span>
                      {uid === user.uid && <SettingsTag>You</SettingsTag>}
                    </div>
                  )}
                </RadioGroup.Option>
              ))}
          </RadioGroup>
        </div>
      )}
    </div>
  );
}

/**
 * The settings pages across the top, instead of the sidebar, in a window too
 * narrow for it.
 */
export function SettingsPageStrip({
  current,
  className,
}: {
  /** the page shown, e.g. "general" */
  current?: string;
  className?: string;
}) {
  const { setPathname } = useContext(BrowserLocationContext);

  return (
    <nav
      aria-label="Settings"
      className={clsx(
        'flex-row items-center gap-1 overflow-x-auto px-3 py-2.5',
        className
      )}
    >
      {pageGroups.flatMap((group) =>
        group.pages.map((setting) => (
          <button
            key={setting.path}
            type="button"
            aria-current={current === setting.path ? 'page' : undefined}
            className={clsx(
              settingsNavItemClass({
                active: current === setting.path,
                activeClass: ACTIVE_CLASS,
              }),
              'w-auto shrink-0'
            )}
            onClick={() => setPathname(`/settings/${setting.path}`)}
          >
            {setting.title}
          </button>
        ))
      )}
    </nav>
  );
}
