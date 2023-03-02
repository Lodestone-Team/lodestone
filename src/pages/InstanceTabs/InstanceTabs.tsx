import { useContext, useEffect, useState } from 'react';
import { InstanceContext } from 'data/InstanceContext';
import { useDocumentTitle } from 'usehooks-ts';
import { useLocation } from 'react-router-dom';
import { InstanceTabListMap, spanMap } from '../../data/GameTypeMappings';
import Label from 'components/Atoms/Label';
import { cn, stateToLabelColor } from 'utils/util';
import Spinner from 'components/DashboardLayout/Spinner';
import { SetupInstanceManifest } from 'data/InstanceGameTypes';
import { Games } from 'bindings/InstanceInfo';
const InstanceTabs = () => {
  useDocumentTitle('Dashboard - Lodestone');
  const location = useLocation();
  const [path, setPath] = useState(location.pathname.split('/')[2]);
  const { selectedInstance: instance } = useContext(InstanceContext);
  const uuid = instance?.uuid;
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    setPath(location.pathname.split('/')[2]);
  }, [location.pathname]);

  useEffect(() => {
    //give time for instance to load
    setTimeout(() => {
      setLoading(false);
    }, 1000);
  }, []);

  if (!instance || !uuid) {
    if (loading) {
      return <Spinner />;
    } else {
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
  }
  const game = Object.keys(instance.game_type)[0] as Games;
  const variant = instance.game_type[game]['variant'];
  const tabs = InstanceTabListMap[game];

  if (!tabs) {
    return (
      <div
        className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="dashboard-instance-heading truncate whitespace-pre">
              Unknown game type {spanMap[game][variant]}
            </h1>
          </div>
        </div>
      </div>
    );
  }

  const tab = tabs.find((tab) => tab.path === path);
  if (!tab) {
    return (
      <div
        className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="dashboard-instance-heading truncate whitespace-pre">
              Unknown tab {path}
            </h1>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        'relative mx-auto flex h-full w-full flex-row justify-center @container',
        tab.width
      )}
      key={uuid}
    >
      <div
        className="gutter-stable -mx-3 flex grow flex-row items-stretch overflow-y-auto pl-4 pr-2"
        key={`${instance.name}-${tab.title}`}
      >
        <div className="flex h-fit min-h-full w-full flex-col gap-8 pt-10 pb-8 focus:outline-none">
          <div className="flex font-title text-h1 font-bold leading-tight text-gray-300">
            {tab.title}
            {tab.title === 'Console' && (
              <Label
                size="medium"
                className="ml-2 mt-[6px]"
                color={stateToLabelColor[instance.state]}
              >
                {instance.state}
              </Label>
            )}
          </div>
          {tab.content}
        </div>
      </div>
    </div>
  );
};

export default InstanceTabs;
