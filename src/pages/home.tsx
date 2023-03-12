import axios from 'axios';
import { useEffect, useState } from 'react';
import { useDocumentTitle } from 'usehooks-ts';
import { useUserAuthorized, useUserInfo, useUserLoggedIn } from 'data/UserInfo';
import { useContext } from 'react';
import Spinner from 'components/DashboardLayout/Spinner';
import Button from 'components/Atoms/Button';
import { InstanceContext } from 'data/InstanceContext';

const Home = () => {
  const { setShowCreateInstance } = useContext(InstanceContext);
  const userLoggedIn = useUserLoggedIn();
  const canCreateInstance = useUserAuthorized('can_create_instance');
  const { isLoading } = useUserInfo();

  useDocumentTitle('Home - Lodestone');

  const [loading, setLoading] = useState(true);

  useEffect(() => {
    //give time for user to load
    setTimeout(() => {
      setLoading(false);
    }, 1000);
  }, []);

  if (loading && isLoading) {
    return <Spinner />;
  }

  return (
    // used to possibly center the content
    <div className="flex flex-col items-center justify-center text-gray-faded/30">
      <div className="flex h-28 w-28 items-center justify-center rounded-full border border-dashed border-gray-faded/30 align-middle">
        <img
          src="/assets/placeholder-cube.png"
          alt="placeholder"
          className="mx-auto w-20"
        />
      </div>
      <div className="mt-4 text-h2 font-extrabold ">Manage your instances</div>
      <div className="text-medium font-bold">
        To start, select an instance from the menu or create a new instance
      </div>
      {userLoggedIn && (
        <div className="mt-5">
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
