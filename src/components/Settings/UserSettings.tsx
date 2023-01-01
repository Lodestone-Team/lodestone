import Button from 'components/Atoms/Button';
import UserBox from 'components/UserBox';
import { SettingsContext } from 'data/SettingsContext';
import { useUserInfo } from 'data/UserInfo';
import { useContext } from 'react';

export const UserSettings = () => {
  const { userList, selectUser, selectedUser } = useContext(SettingsContext);
  const { data: userInfo } = useUserInfo();
  const canManageUsers = userInfo?.is_owner || false;

  // TODO: add non-owner view (can't manage users, can only change their own password)
  return (
    <div className="flex w-full flex-col gap-4 @4xl:flex-row">
      <div className="flex  w-full flex-row flex-nowrap items-end justify-between gap-4 @4xl:w-[28rem] @4xl:flex-col @4xl:items-start">
        <div>
          <h1 className="text-large font-black">
            All Members ({Object.keys(userList).length})
          </h1>
          <h2 className="text-base font-medium italic tracking-tight text-white/50">
            A list of all users. Click into a user to manage.
          </h2>
        </div>
        <Button
          label="Create New User"
          className="whitespace-nowrap"
          onClick={() => {
            // TODO
            alert('TODO');
          }}
        />
      </div>
      <div className="w-full h-fit rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
        {Object.keys(userList).map((uid) => (
          <UserBox
            user={userList[uid]}
            key={uid}
            onClick={() => selectUser(userList[uid])}
          />
        ))}
      </div>
    </div>
  );
};

export default UserSettings;
