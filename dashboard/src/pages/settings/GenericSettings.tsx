import { useCoreInfo } from 'data/SystemInfo';
import { useDocumentTitle } from 'usehooks-ts';
import packageJson from '../../../package.json';

export const GenericSetting = () => {
  useDocumentTitle('Lodestone Generic Settings - Lodestone');
  const { data: coreInfo } = useCoreInfo();

  return (
    <>
      <div className="relative mx-auto flex h-full w-full max-w-2xl flex-col @container ">
        <div className="flex w-full flex-col gap-12 overflow-y-scroll px-4 pt-14">
          <h1 className="dashboard-instance-heading">Version Info</h1>
          <div className="flex w-full flex-col gap-4 @4xl:flex-row">
            <div className="w-[28rem]">
              <h2 className="text-h2 font-bold tracking-medium">
                
              </h2>
            </div>
            <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
                <div
                    className={`flex flex-row justify-between group relative gap-4 bg-gray-800 px-4 py-3 text-medium h-fit`}
                >
                    <div className={`flex min-w-0 grow flex-col`}>
                        <div className="flex row">
                        <label className="text-medium font-medium text-gray-300">Dashboard version</label>
                        </div>
                    </div>
                    {packageJson.version}
                </div>
                <div
                    className={`flex flex-row items-center justify-between group relative gap-4 bg-gray-800 px-4 py-3 text-medium`}
                >
                    <div className={`flex min-w-0 grow flex-col`}>
                        <div className="flex row">
                        <label className="text-medium font-medium text-gray-300">Core version</label>
                        </div>
                    </div>
                    {coreInfo ? coreInfo.version : 'unavailable'}
                </div>
            </div>
          </div>
        </div>
      </div>
    </>
  );
};

export default GenericSetting;
