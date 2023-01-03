import { useContext } from 'react';
import Button from 'components/Atoms/Button';
import { faXmark } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { RadioGroup } from '@headlessui/react';
import { SettingsContext } from 'data/SettingsContext';
import Avatar from 'components/Atoms/Avatar';
import clsx from 'clsx';
import { useUid } from 'data/UserInfo';

export default function SettingsLeftNav({ className }: { className?: string }) {
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const { userList, selectedUser, selectUser, tabIndex } =
    useContext(SettingsContext);
  const uid = useUid();
  return (
    <div className={`flex w-full flex-col items-center px-4 ${className}`}>
      <div className="flex h-full w-full grow flex-col items-start gap-4 pt-12 pb-4">
        {selectedUser ? (
          <Button
            label="Unselect User"
            icon={faXmark}
            onClick={() => {
              selectUser(undefined);
            }}
            className="ml-2"
          />
        ) : (
          <Button
            label="Close Settings"
            icon={faXmark}
            onClick={() => {
              setSearchParam('user', undefined);
              setPathname('/');
            }}
            className="ml-2"
          />
        )}
        {selectedUser && (
          <RadioGroup
            value={selectedUser}
            onChange={selectUser}
            className="-mx-1.5 flex min-h-0 w-full flex-col items-stretch gap-1 overflow-y-auto px-3 pt-1"
          >
            <RadioGroup.Label className="sr-only">
              Select a user
            </RadioGroup.Label>
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
                        'flex flex-row items-center justify-start gap-1.5',
                        checked
                          ? 'bg-gray-800 outline outline-1 outline-white/50'
                          : 'bg-gray-850'
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
        )}
      </div>
    </div>
  );
}
