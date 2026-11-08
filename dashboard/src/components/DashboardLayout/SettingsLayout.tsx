import { PublicUser } from 'bindings/PublicUser';
import ResizePanel from 'components/Atoms/ResizePanel';
import { useAllUsers } from 'data/AllUsers';
import { SettingsContext } from 'data/SettingsContext';
import { useUserInfo } from 'data/UserInfo';
import { Suspense, useState } from 'react';
import { Outlet } from 'react-router';
import { useLocalStorage } from 'usehooks-ts';
import { useQueryParam } from 'utils/hooks';
import SettingsLeftNav from './SettingsLeftNav';
import Spinner from './Spinner';

export const SettingsLayout = () => {
  const { data: userInfo, isPending: userInfoPending } = useUserInfo();
  const canManageUsers = userInfo?.is_owner || false;
  const { data: dataUserList, isPending: userListPending } =
    useAllUsers(canManageUsers);
  const [tabIndex, setTabIndex] = useState(0);
  const [leftNavSize, setLeftNavSize] = useLocalStorage('leftNavSize', 220);
  /* Start userList */
  const [queryUid, setQueryUid] = useQueryParam('user', '');
  const userList = canManageUsers ? dataUserList : undefined;
  // Derived, not copied into state by an effect, which was a render behind:
  // opening a user (or a link to one) first showed the list of all users.
  const selectedUser: PublicUser | null =
    (queryUid && userList?.[queryUid]) || null;

  function selectUser(user: PublicUser | null) {
    setQueryUid(user === null ? '' : user.uid);
  }
  /* End userList */

  return (
    <SettingsContext.Provider
      value={{
        selectedUser,
        selectUser,
        userList: userList || {},
        userListLoading: userInfoPending || (canManageUsers && userListPending),
        tabIndex,
        setTabIndex,
      }}
    >
      <ResizePanel
        direction="e"
        maxSize={280}
        minSize={200}
        size={leftNavSize}
        validateSize={false}
        onResize={setLeftNavSize}
        containerClassNames="min-h-0"
      >
        <SettingsLeftNav className="border-r border-fade-700 bg-gray-850" />
      </ResizePanel>
      <div className="h-full min-w-0 grow child:h-full">
        <Suspense fallback={<Spinner />}>
          <Outlet />
        </Suspense>
      </div>
    </SettingsContext.Provider>
  );
};
