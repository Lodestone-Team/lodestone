import { useContext, useEffect, useState } from 'react';
import { InstanceContext } from 'data/InstanceContext';
import { useDocumentTitle } from 'usehooks-ts';
import { useLocation } from 'react-router-dom';
import InstanceTabListMap from '../../data/InstanceTabListMap';
import Label from 'components/Atoms/Label';
import { stateToLabelColor } from 'utils/util';
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
      return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-gray-900 bg-opacity-75">
          <div className="relative h-24 w-24">
            <div
              className="absolute top-0 left-0 h-full w-full animate-spin rounded-full border-4 border-t-4 border-blue-400"
              style={{
                borderBottomColor: 'transparent',
              }}
            ></div>
          </div>
        </div>
      );
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
          )
      )}
    </div>
  );
};

export default InstanceTabs;
