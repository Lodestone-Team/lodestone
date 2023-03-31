import { useContext, useState, useEffect } from 'react';
import Button from 'components/Atoms/Button';
import { faXmark, faServer, faRightToBracket, faUser } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { RadioGroup } from '@headlessui/react';
import { SettingsContext } from 'data/SettingsContext';
import Avatar from 'components/Atoms/Avatar';
import clsx from 'clsx';
import { useUid } from 'data/UserInfo';
import { AccountSettingsTabList, CoreSettingsTabList } from '../../../pages';




export default function SettingsLeftNav({ className }: { className?: string }) {
  const [setActive, setActiveTab] = useState(location.pathname.split('/')[2]);
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const { userList, selectedUser, selectUser} =
  useContext(SettingsContext);
  const uid = useUid();

  useEffect(() => {
    setActiveTab(location.pathname.split('/')[2]);
  }, [location.pathname]);

  return (
    <div className={`flex w-full flex-col items-center px-4 ${className}`}>
      <div className="flex h-full w-full grow flex-col items-start gap-4 pt-12 pb-4">
      <button
                className={clsx(
                  'flex flex-row items-center gap-x-1.5',
                  'cursor-pointer rounded-md py-1 px-2',
                  'text-medium font-medium leading-5 tracking-normal',
                  'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-blue-faded/50',
                )}
                onClick={() => {
              setSearchParam('user', undefined);
              const returnRoute = localStorage.getItem('lastVisitedRoute');
              setPathname(returnRoute || '/');
            }}
        >
        <div className='text-gray-faded/30 pr-1'>
          <FontAwesomeIcon icon={faRightToBracket} rotation={180} size='xl' />
        </div>
        <h2 className="text-h2 font-extrabold tracking-medium">Settings</h2>
      </button>

        <RadioGroup
          className={`mx-1 flex min-h-0 flex-col gap-y-1 overflow-y-auto px-1 py-1 child:w-full w-full`}
        >
          <RadioGroup.Label className="text-small font-bold leading-snug text-gray-faded/30 flex flex-row items-center gap-x-1.5">
            <FontAwesomeIcon icon={faServer} />
            CORE
          </RadioGroup.Label>
          {CoreSettingsTabList.map((setting) => (
          <RadioGroup.Option
              key={setting.path}
              value={`/settings/${setting.path}`}
              className="rounded-md outline-none focus-visible:bg-gray-800 child:w-full"
            >
              <button
                className={clsx(
                  'flex flex-row items-center gap-x-1.5',
                  'cursor-pointer rounded-md py-1 px-2',
                  'text-medium font-medium leading-5 tracking-normal',
                  'hover:bg-gray-800',
                  'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-blue-faded/50',
                  setActive === setting.path
                    ? 'bg-gray-800 outline outline-1 outline-fade-700'
                    : ''
                )}
                onClick={() => setPathname(`/settings/${setting.path}`)}
              >
                <div className="text-gray-300">{setting.title}</div>
              </button>
            </RadioGroup.Option>
          ))}
          <RadioGroup.Label className="text-small font-bold leading-snug text-gray-faded/30 flex flex-row items-center gap-x-1.5 mt-2">
            <FontAwesomeIcon icon={faUser} />
            ACCOUNT
          </RadioGroup.Label>
          {AccountSettingsTabList.map((setting) => (
          <RadioGroup.Option
              key={setting.path}
              value={`/settings/${setting.path}`}
              className="rounded-md outline-none focus-visible:bg-gray-800 child:w-full"
            >
              <button
                className={clsx(
                  'flex flex-row items-center gap-x-1.5',
                  'cursor-pointer rounded-md py-1 px-2',
                  'text-medium font-medium leading-5 tracking-normal',
                  'hover:bg-gray-800',
                  'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-blue-faded/50',
                  setActive === setting.path
                    ? 'bg-gray-800 outline outline-1 outline-fade-700'
                    : ''
                )}
                onClick={() => setPathname(`/settings/${setting.path}`)}
              >
                <div className="text-gray-300">{setting.title}</div>
              </button>
            </RadioGroup.Option>
          ))}

        </RadioGroup>
          {(
          <Button
            label="Unselect User"
            icon={faXmark}
            onClick={() => {
              selectUser(null);
            }}
            className="ml-2"
          />
        )}
        {
          <RadioGroup
            value={selectedUser}
            onChange={selectUser}
            className="-mx-1.5 flex min-h-0 w-full flex-col items-stretch gap-1 overflow-y-auto px-3 py-1"
          >
            {Object.values(userList)
              .sort((a, b) =>
                a.uid === uid
                  ? -1
                  : b.uid === uid
                  ? 1
                  : a.username.localeCompare(b.username)
              )
              .map((user) => (
                <RadioGroup.Option key={user.uid} value={user}>
                  {({ checked }) => (
                    <div
                      className={clsx(
                        'w-full cursor-pointer rounded-md py-1.5 px-3 hover:bg-gray-700',
                        'flex flex-row items-center justify-start gap-1.5 text-medium',
                        checked
                          ? 'bg-gray-800 outline outline-1 outline-white/50'
                          : ''
                      )}
                    >
                      <Avatar name={user.uid} />
                      <span className="truncate">
                        {user.username} {uid === user.uid && '(You)'}
                      </span>
                    </div>
                  )}
                </RadioGroup.Option>
              ))}
          </RadioGroup>
        }
      </div>
    </div>
  );
}
