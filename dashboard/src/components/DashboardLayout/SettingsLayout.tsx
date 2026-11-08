import { Dialog, Transition } from '@headlessui/react';
import { faXmark } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { PublicUser } from 'bindings/PublicUser';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
import { useAllUsers } from 'data/AllUsers';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { SettingsContext } from 'data/SettingsContext';
import { useUserInfo } from 'data/UserInfo';
import {
  Fragment,
  Suspense,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react';
import { useQueryParam } from 'utils/hooks';
import { forgetSettingsReturn, settingsReturnPath } from 'utils/settingsReturn';
import { AccountSettingsTabList, CoreSettingsTabList } from '../../../pages';
import SettingsLeftNav, { SettingsPageStrip } from './SettingsLeftNav';
import { PageLoading } from './LayoutSkeletons';

const pages = [...CoreSettingsTabList, ...AccountSettingsTabList];

/**
 * Settings, in a large modal over the dashboard: the pages listed on the left
 * and the current page on the right. Each page has its own route
 * (/settings/general, ...), so links to them work; closing goes back to the
 * page settings was opened from. In a small window the modal fills it.
 */
export const SettingsLayout = ({
  open,
  onClosed,
}: {
  /** on a settings page */
  open: boolean;
  /** the closing animation has finished */
  onClosed: () => void;
}) => {
  const { location, setPathname, setSearchParam } = useContext(
    BrowserLocationContext
  );
  const { data: userInfo, isPending: userInfoPending } = useUserInfo();
  const canManageUsers = userInfo?.is_owner || false;
  const { data: dataUserList, isPending: userListPending } =
    useAllUsers(canManageUsers);
  const [tabIndex, setTabIndex] = useState(0);
  /* Start userList */
  // not written to the URL while empty: the modal stays mounted while it
  // closes, and must not add `?user=` to the page it closes to
  const [queryUid, setQueryUid] = useQueryParam('user', '', false);
  const userList = canManageUsers ? dataUserList : undefined;
  // Derived, not copied into state by an effect, which was a render behind:
  // opening a user (or a link to one) first showed the list of all users.
  const selectedUser: PublicUser | null =
    (queryUid && userList?.[queryUid]) || null;

  function selectUser(user: PublicUser | null) {
    setQueryUid(user === null ? '' : user.uid);
  }
  /* End userList */

  // the page shown: kept while the modal closes, so it doesn't go blank
  const [shownPath, setShownPath] = useState(location.pathname);
  if (open && shownPath !== location.pathname) setShownPath(location.pathname);
  const page = pages.find((tab) => `/settings/${tab.path}` === shownPath);

  const close = () => {
    const returnTo = settingsReturnPath() ?? '/';
    setSearchParam('user', undefined);
    setPathname(returnTo);
    forgetSettingsReturn();
  };

  // A click on a toast is outside the modal, but shouldn't close it
  const lastPointerDown = useRef<Element | null>(null);
  useEffect(() => {
    const remember = (event: PointerEvent) => {
      lastPointerDown.current =
        event.target instanceof Element ? event.target : null;
    };
    document.addEventListener('pointerdown', remember, true);
    return () => document.removeEventListener('pointerdown', remember, true);
  }, []);

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
      <Transition appear show={open} as={Fragment} afterLeave={onClosed}>
        <Dialog
          onClose={() => {
            if (lastPointerDown.current?.closest('.Toastify')) return;
            close();
          }}
          className="relative z-50"
        >
          <ModalBackdrop strong />
          <div className="fixed inset-0 flex items-center justify-center p-12 max-[760px]:p-0 [@media(max-height:559px)]:p-0">
            <ModalPanel className="relative flex size-full max-w-270 flex-row overflow-hidden rounded-2xl border border-fade-700 bg-gray-900 drop-shadow-xl max-[760px]:max-w-none max-[760px]:rounded-none max-[760px]:border-0 [@media(max-height:559px)]:max-w-none [@media(max-height:559px)]:rounded-none [@media(max-height:559px)]:border-0">
              <SettingsLeftNav
                current={page?.path}
                className="w-58 shrink-0 border-r border-fade-700 bg-gray-875 max-[760px]:hidden"
              />
              <div className="flex min-w-0 grow flex-col">
                <SettingsPageStrip
                  current={page?.path}
                  className="hidden shrink-0 border-b border-fade-700 bg-gray-875 pr-14 max-[760px]:flex"
                />
                <div className="min-h-0 grow child:h-full">
                  <Suspense fallback={<PageLoading className="max-w-170" />}>
                    {page?.content}
                  </Suspense>
                </div>
              </div>
              <button
                type="button"
                aria-label="Close settings"
                title="Close (Esc)"
                onClick={close}
                className="absolute top-3 right-3 flex size-8 items-center justify-center rounded-lg text-gray-500 hover:bg-gray-800 hover:text-gray-300 focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden"
              >
                <FontAwesomeIcon icon={faXmark} className="size-4" />
              </button>
            </ModalPanel>
          </div>
        </Dialog>
      </Transition>
    </SettingsContext.Provider>
  );
};
