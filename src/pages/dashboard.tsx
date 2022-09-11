import { faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import ClipboardTextfield from 'components/ClipboardTextfield';
import Label from 'components/Label';
import { useInstanceList } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import type { NextPage } from 'next';
import { useRouter } from 'next/router';
import { useContext, useEffect, useMemo, useState } from 'react';
import { stateToLabelColor } from 'utils/util';

const Dashboard: NextPage = () => {
  const router = useRouter();
  const lodestoneContex = useContext(LodestoneContext);
  const { uuid: queryUuid } = router.query;
  const [uuid, setUuid] = useState('');
  const { data: instances } = useInstanceList();
  const instance = useMemo(() => {
    if (uuid) return instances?.[uuid];
  }, [uuid, instances]);

  // if no uuid, redirect to /
  useEffect(() => {
    if (!router.isReady) return;
    if (!queryUuid) {
      router.push(
        {
          pathname: '/',
          query: router.query,
        },
        undefined,
        { shallow: true }
      );
    } else {
      // set uuid to queryuuid
      // query uuid could be a string[] for some reason, just take the first one
      if (Array.isArray(queryUuid)) {
        setUuid(queryUuid[0]);
      } else {
        setUuid(queryUuid);
      }
    }
  }, [queryUuid, router.isReady, router]);

  if (!uuid) return <></>;
  // TODO: add loading state, don't let it flash blank

  if (!instance) {
    return (
      <div className="px-12 py-10 bg-gray-800">
        <h1 className="-ml-4 font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
          Instance not found
        </h1>
      </div>
    );
  }

  const labelColor = stateToLabelColor[instance.state];

  return (
    <div className="px-12 py-10 bg-gray-800">
      <div className="flex flex-col items-start gap-4">
        <div className="flex flex-row items-center gap-10">
          <div className="flex flex-row items-center gap-4">
            {/* TODO: create a universal "text with edit button" component */}
            <h1 className="-ml-4 font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
              {instance?.name}
            </h1>
            <FontAwesomeIcon
              className="text-gray-500 text-medium"
              icon={faPenToSquare}
            />
          </div>
          <div className="flex flex-row items-center gap-4">
            {/* TODO: create a universal game flavour image component */}
            <img
              src="/assets/minecraft-vanilla.png"
              alt={`${instance.game_type} logo`}
              className="w-8 h-8"
            />
            <Label size="large" color={labelColor}>
              {instance.state}
            </Label>
          </div>
        </div>
        <div className="flex flex-row items-center gap-4">
          <Label size="large" color={labelColor}>
            Player Count {instance.player_count}/{instance.max_player_count}
          </Label>
          <Label
            size="large"
            color="gray"
            className="flex flex-row items-center gap-3"
          >
            <ClipboardTextfield
              text={`${lodestoneContex.address}:${instance.port}`}
              textToCopy={lodestoneContex.address}
            />
          </Label>
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
