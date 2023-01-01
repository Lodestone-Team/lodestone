import { InstanceInfo } from 'bindings/InstanceInfo';
import { PublicUser } from 'bindings/PublicUser';
import { useAllUsers } from 'data/AllUsers';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { InstanceContext } from 'data/InstanceContext';
import { useInstanceList } from 'data/InstanceList';
import { SettingsContext } from 'data/SettingsContext';
import { useUserAuthorized, useUserInfo, useUserLoggedIn } from 'data/UserInfo';
import { useContext, useEffect, useState } from 'react';
import { Outlet } from 'react-router-dom';
import { useQueryParam } from 'utils/hooks';
import LeftNav from './LeftNav';
import SettingsLeftNav from './SettingsLeftNav';

export const SettingsLayout = () => {
  const { setPathname } = useContext(BrowserLocationContext);
  const { data: userInfo } = useUserInfo();
  const canManageuserList = userInfo?.is_owner || false;
  const { data: dataUserList } = useAllUsers(canManageuserList);

  /* Start userList */
  const [queryUid, setQueryUid] = useQueryParam('user', '');
  const [selectedUser, setSelectedUser] = useState<PublicUser | undefined>(
    undefined
  );
  const userList = canManageuserList ? dataUserList : undefined;

  useEffect(() => {
    if (queryUid && userList && queryUid in userList)
      setSelectedUser(userList[queryUid]);
    else setSelectedUser(undefined);
  }, [userList, queryUid]);

  function selectUser(user?: PublicUser) {
    console.log('selectUser', user);
    if (user === undefined) {
      setSelectedUser(undefined);
      setQueryUid('');
    } else {
      setSelectedUser(user);
      setQueryUid(user.uid);
    }
  }
  /* End userList */

  return (
    <SettingsContext.Provider
      value={{
        selectedUser,
        selectUser,
        userList: userList || {},
      }}
    >
      <div className="flex grow flex-row justify-center gap-[1vw]">
        <div className="flex h-full grow basis-60 flex-row flex-nowrap items-stretch justify-end">
          <div className="h-full w-[16rem] max-w-[16rem] child:h-full">
            <SettingsLeftNav />
          </div>
        </div>
        <div className="h-full min-w-0 grow basis-[1024px] child:h-full">
          <div className="max-w-[1024px]">
            <Outlet />
          </div>
        </div>
      </div>
    </SettingsContext.Provider>
  );
};
