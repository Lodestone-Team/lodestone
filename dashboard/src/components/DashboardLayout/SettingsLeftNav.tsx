import { useContext, useMemo } from 'react';
import {
  faServer,
  faRightToBracket,
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
import SidebarFooter from './SidebarFooter';
import { settingsNavItemClass } from 'components/Settings/SettingsToc';
import { SettingsLink, SettingsTag } from 'components/Settings/SettingsBlocks';

// the highlight of the current page: the sidebar is gray-850, one step lighter
const ACTIVE_CLASS =
  'bg-gray-800 text-gray-300 light:outline-1 light:outline-fade-700 light:outline-solid';

/** A heading over a group of the list, e.g. "Core" */
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

export default function SettingsLeftNav({ className }: { className?: string }) {
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const { userList, selectedUser, selectUser } = useContext(SettingsContext);
  const setActive = useMemo(
    () => location.pathname.split('/')[2],
    [location.pathname]
  );
  const uid = useUid();
  const users = Object.values(userList);

  const pageGroups = [
    { label: 'Core', icon: faServer, pages: CoreSettingsTabList },
    { label: 'Account', icon: faUser, pages: AccountSettingsTabList },
  ];

  return (
    <div
      className={clsx(
        'flex size-full flex-col items-center overflow-y-auto px-3',
        className
      )}
    >
      <div className="flex w-full grow flex-col items-start gap-6 pt-12 pb-4">
        <button
          className={clsx(
            'flex flex-row items-center gap-x-1.5',
            'cursor-pointer rounded-md px-2 py-1',
            'text-medium leading-5 font-medium tracking-normal',
            'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden'
          )}
          onClick={() => {
            setSearchParam('user', undefined);
            const returnRoute = localStorage.getItem('lastVisitedRoute');
            setPathname(returnRoute || '/');
          }}
        >
          <div className="pr-1 text-gray-faded/30">
            <FontAwesomeIcon icon={faRightToBracket} rotation={180} size="xl" />
          </div>
          <h2 className="text-h2 font-extrabold tracking-medium">Settings</h2>
        </button>

        <nav aria-label="Settings" className="flex w-full flex-col gap-4">
          {pageGroups.map((group) => (
            <div key={group.label} className="flex flex-col gap-0.5">
              <GroupLabel icon={group.icon}>{group.label}</GroupLabel>
              {group.pages.map((setting) => (
                <button
                  key={setting.path}
                  type="button"
                  aria-current={setActive === setting.path ? 'page' : undefined}
                  className={settingsNavItemClass({
                    active: setActive === setting.path,
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
                disabled={selectedUser === null}
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
              className="flex min-h-0 w-full flex-col items-stretch gap-0.5 overflow-y-auto"
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
      <SidebarFooter showSettings={false} />
    </div>
  );
}
