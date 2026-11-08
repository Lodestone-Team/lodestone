import axios from 'axios';
import { useDocumentTitle } from 'usehooks-ts';
import { useUserAuthorized, useUserInfo, useUserLoggedIn } from 'data/UserInfo';
import { useContext } from 'react';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import Button from 'components/Atoms/Button';
import { InstanceContext } from 'data/InstanceContext';
import { preloadCreateInstanceProps } from 'components/DashboardLayout/CreateInstanceFlow';

const Home = () => {
  useDocumentTitle('Home - Lodestone');
  const { setShowCreateInstance } = useContext(InstanceContext);
  const userLoggedIn = useUserLoggedIn();
  const canCreateInstance = useUserAuthorized('can_create_instance');
  // isLoading is only true while the user info is actually being fetched, so
  // a logged-out user (query disabled) sees the page right away
  const { isLoading } = useUserInfo();

  if (isLoading) {
    // the same layout as the page below
    return (
      <SkeletonGroup className="flex flex-col items-center justify-center">
        <Skeleton className="size-28 rounded-full" />
        <Skeleton className="mt-4 h-6 w-64" />
        <Skeleton className="mt-2 h-4 w-96 max-w-full rounded-sm" />
        <Skeleton className="mt-5 h-8 w-40" />
      </SkeletonGroup>
    );
  }

  return (
    // used to possibly center the content
    <div className="relative flex flex-col items-center justify-center overflow-auto text-gray-faded/30 light:text-ink-muted">
      <div className="flex h-28 w-28 items-center justify-center rounded-full border border-dashed border-gray-faded/30 align-middle">
        <img
          src="/assets/placeholder-cube.png"
          alt="placeholder"
          className="mx-auto w-20"
          style={{ imageRendering: 'pixelated' }}
        />
      </div>
      <div className="mt-4 text-h2 font-extrabold ">Manage your instances</div>
      <div className="text-medium font-bold">
        To start, select an instance from the menu or create a new instance
      </div>
      {userLoggedIn && (
        <div className="mt-5" {...preloadCreateInstanceProps}>
          <Button
            align="start"
            labelGrow={true}
            className="w-full"
            label="Create a new instance"
            onClick={() => setShowCreateInstance(true)}
            disabled={!canCreateInstance}
          />
        </div>
      )}
    </div>
  );
};

export default Home;
