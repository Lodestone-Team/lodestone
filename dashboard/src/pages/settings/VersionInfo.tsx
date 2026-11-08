import { useCoreInfo } from 'data/SystemInfo';
import { useDocumentTitle } from 'usehooks-ts';
import { DASHBOARD_VERSION } from 'utils/version';

const VersionRow = ({ label, value }: { label: string; value: string }) => (
  <div className="relative flex flex-row items-center justify-between gap-4 bg-gray-800 px-4 py-3 text-medium">
    <label className="text-medium font-medium text-gray-300">{label}</label>
    {value}
  </div>
);

export const VersionInfo = () => {
  useDocumentTitle('Lodestone Version Info - Lodestone');
  const { data: coreInfo } = useCoreInfo();

  return (
    <div className="@container relative mx-auto flex h-full w-full max-w-2xl flex-col">
      <div className="flex w-full flex-col gap-12 overflow-y-scroll px-4 pt-8">
        <h1 className="dashboard-instance-heading">Version Info</h1>
        <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0">
          <VersionRow label="Dashboard version" value={DASHBOARD_VERSION} />
          <VersionRow
            label="Core version"
            value={coreInfo ? coreInfo.version : 'unavailable'}
          />
        </div>
      </div>
    </div>
  );
};

export default VersionInfo;
