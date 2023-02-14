import { useContext, useEffect, useState } from 'react';
import { InstanceContext } from 'data/InstanceContext';
import { useDocumentTitle } from 'usehooks-ts';
import { useLocation } from 'react-router-dom';
import InstanceTabListMap from '../../data/InstanceTabListMap';
const InstanceTabs = () => {
  useDocumentTitle('Dashboard - Lodestone');
  const location = useLocation();
  const [path, setPath] = useState(location.pathname.split('/')[2]);
  const { selectedInstance: instance } = useContext(InstanceContext);
  const uuid = instance?.uuid;

  useEffect(() => {
    setPath(location.pathname.split('/')[2]);
  }, [location.pathname]);

  if (!instance || !uuid) {
    return (
      <div
        className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="dashboard-instance-heading truncate whitespace-pre">
              Instance not found
            </h1>
          </div>
        </div>
      </div>
    );
  }
  return (
    <div
      className="relative mx-auto flex h-full w-full max-w-4xl flex-row justify-center @container"
      key={uuid}
    >
      {InstanceTabListMap[instance.game_type].map(
        (tab) =>
          tab.path === path && (
            <div
              className="gutter-stable -mx-4 flex grow flex-row items-stretch overflow-y-auto pl-4 pr-2"
              key={`${instance.name}-${tab.title}`}
            >
              <div className="flex h-fit min-h-full w-full flex-col gap-16 pt-6 pb-10 focus:outline-none">
                {tab.title}
                {tab.content}
              </div>
            </div>
          )
      )}
    </div>
  );
};

export default InstanceTabs;
