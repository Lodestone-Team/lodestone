import { Tab } from '@headlessui/react';
import ClipboardTextfield from 'components/ClipboardTextfield';
import GameConsole from 'components/GameConsole';
import DashboardCard from 'components/DashboardCard';
import Label from 'components/Atoms/Label';
import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { useContext, useEffect, useState } from 'react';
import { axiosPutSingleValue, stateToLabelColor } from 'utils/util';
import EditableTextfield from 'components/EditableTextfield';
import { useQueryClient } from '@tanstack/react-query';
import InstanceGeneralCard from 'components/Instance/InstanceGeneralCard';
import InstanceSettingCard from 'components/Instance/InstanceSettingCard';
import InstancePerformanceCard from 'components/Instance/InstancePerformanceCard';
import FileViewer from 'components/FileViewer/FileViewer';
import { InstanceContext } from 'data/InstanceContext';
import GameIcon from 'components/Atoms/GameIcon';
import { useGlobalSettings } from 'data/GlobalSettings';
import { toast } from 'react-toastify';
import LoadingStatusIcon from 'components/Atoms/LoadingStatusIcon';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faCompass } from '@fortawesome/free-solid-svg-icons';
import { useDocumentTitle } from 'usehooks-ts';

/*CURRENTLY NOT BEING USED*/

const Dashboard = () => {
  // useDocumentTitle('Dashboard - Lodestone');
  // const { core } = useContext(LodestoneContext);
  // const { address } = core;
  // const { selectedInstance: instance } = useContext(InstanceContext);
  // const { data: globalSettings } = useGlobalSettings();
  // const domain = (globalSettings?.domain ?? address) || 'localhost';
  // const queryClient = useQueryClient();
  // const uuid = instance?.uuid;
  // const [selectedTabIndex, setSelectedTabIndex] = useState(0);
  // if (!instance || !uuid) {
  //   return (
  //     <div
  //       className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
  //       key={uuid}
  //     >
  //       <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
  //         <div className="flex min-w-0 flex-row items-center gap-4">
  //           <h1 className="dashboard-instance-heading truncate whitespace-pre">
  //             Instance not found
  //           </h1>
  //         </div>
  //       </div>
  //     </div>
  //   );
  // }
  // const labelColor = stateToLabelColor[instance.state];
  // // tablist is map from GameType to tabs
  // const tabList = {
  //   minecraft: [
  //     {
  //       title: 'Console',
  //       content: <GameConsole />,
  //     },
  //     {
  //       title: 'Settings',
  //       content: (
  //         <>
  //           <InstanceGeneralCard />
  //           <InstanceSettingCard />
  //         </>
  //       ),
  //     },
  //     {
  //       title: 'Files',
  //       content: <FileViewer />,
  //     },
  //     {
  //       title: 'Performance',
  //       content: (
  //         <>
  //           <InstancePerformanceCard />
  //         </>
  //       ),
  //     },
  //     {
  //       title: 'Resources',
  //       content: (
  //         <DashboardCard className="grow !justify-center !gap-4">
  //           <img
  //             src="/assets/placeholder-cube.png"
  //             alt="placeholder"
  //             className="mx-auto w-20"
  //           />
  //           <p className="text-center text-h3 font-medium text-white/50">
  //             Coming soon to a dashboard near you!
  //           </p>
  //         </DashboardCard>
  //       ),
  //     },
  //     {
  //       title: 'Macro',
  //       content: (
  //         <DashboardCard className="grow !justify-center !gap-4">
  //           <img
  //             src="/assets/placeholder-cube.png"
  //             alt="placeholder"
  //             className="mx-auto w-20"
  //           />
  //           <p className="text-center text-h3 font-medium text-white/50">
  //             Coming soon to a dashboard near you!
  //           </p>
  //         </DashboardCard>
  //       ),
  //     },
  //   ],
  // };
  // const setInstanceName = async (name: string) => {
  //   await axiosPutSingleValue<void>(`/instance/${uuid}/name`, name);
  //   updateInstance(uuid, queryClient, (oldData) => ({
  //     ...oldData,
  //     name,
  //   }));
  // };
  // // might use this later
  // // eslint-disable-next-line @typescript-eslint/no-unused-vars
  // const setInstanceDescription = async (description: string) => {
  //   await axiosPutSingleValue<void>(
  //     `/instance/${uuid}/description`,
  //     description
  //   );
  //   updateInstance(uuid, queryClient, (oldData) => ({
  //     ...oldData,
  //     description,
  //   }));
  // };
  // return (
  //   <div
  //     className="relative mx-auto flex h-full w-full max-w-2xl flex-row justify-center @container"
  //     key={uuid}
  //   >
  //     {/* main content container */}
  //     <div className="flex w-full grow flex-col items-stretch gap-2 px-4 pt-8">
  //       <div className="flex w-full min-w-0 flex-row items-center gap-4">
  //         {/* TODO: create a universal "text with edit button" component */}
  //         <EditableTextfield
  //           initialText={instance.name}
  //           type={'heading'}
  //           onSubmit={setInstanceName}
  //           placeholder="No name"
  //           containerClassName="min-w-0"
  //         />
  //       </div>
  //       <div className="-mt-2 mb-2 flex flex-row flex-wrap items-center gap-4">
  //         <GameIcon
  //           game_type={instance.game_type}
  //           game_flavour={instance.flavour}
  //           className="h-6 w-6"
  //         />
  //         <Label size="large" color={labelColor}>
  //           {instance.state}
  //         </Label>
  //         <Label size="large" color={labelColor}>
  //           Player Count {instance.player_count}/{instance.max_player_count}
  //         </Label>
  //         <ClipboardTextfield text={`${domain}:${instance.port}`} />
  //       </div>
  //       {/* <div className="flex w-full flex-row items-center gap-2">
  //         <EditableTextfield
  //           initialText={instance.description}
  //           type={'description'}
  //           onSubmit={setInstanceDescription}
  //           placeholder="No description"
  //           containerClassName="min-w-0"
  //         />
  //       </div> */}
  //       <Tab.Group
  //         selectedIndex={selectedTabIndex}
  //         onChange={setSelectedTabIndex}
  //       >
  //         <Tab.List className="flex w-full flex-row flex-wrap items-center gap-4 border-b-2 border-gray-700">
  //           {tabList[instance.game_type].map((tab) => (
  //             <Tab
  //               key={tab.title}
  //               className={({ selected }) =>
  //                 `text-h3 font-bold tracking-medium focus-visible:outline-none ${
  //                   selected
  //                     ? 'border-b-2 border-blue-200 text-blue-200'
  //                     : 'mb-0.5 text-gray-500'
  //                 }`
  //               }
  //             >
  //               {tab.title}
  //             </Tab>
  //           ))}
  //         </Tab.List>
  //         {/* gutter-stable basically adds 8px of padding on the right */}
  //         <Tab.Panels className="gutter-stable -mx-4 flex grow flex-row items-stretch overflow-y-auto pl-4 pr-2">
  //           {tabList[instance.game_type].map((tab) => (
  //             <Tab.Panel
  //               className="flex h-fit min-h-full w-full flex-col gap-8 pt-10 pb-8 focus:outline-none"
  //               key={tab.title}
  //             >
  //               {tab.content}
  //             </Tab.Panel>
  //           ))}
  //         </Tab.Panels>
  //       </Tab.Group>
  //     </div>
  //   </div>
  // );
};

export default Dashboard;
