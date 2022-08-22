import { faClone, faPenToSquare } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import ClipboardTextfield from 'components/ClipboardTextfield';
import Label from 'components/Label';
import { InstanceState, selectInstanceList } from 'data/InstanceList';
import type { NextPage } from 'next';
import Head from 'next/head';
import Image from 'next/image';
import { useRouter } from 'next/router';
import { useEffect, useState } from 'react';
import { useAppDispatch, useAppSelector } from 'utils/hooks';
import { statusToLabelColor } from 'utils/util';

const Dashboard: NextPage = () => {
  const router = useRouter();
  const { uuid: queryUuid } = router.query;
  const [uuid, setUuid] = useState('');
  const dispatch = useAppDispatch();
  const instanceListState = useAppSelector(selectInstanceList);
  const instance = instanceListState.instances[uuid];

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
  }, [queryUuid, router]);

  if (!instance) {
    return (
      <div className="px-12 py-10 bg-gray-800">
        <h1 className="-ml-4 font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
          Instance not found
        </h1>
      </div>
    );
  }

  const labelColor = statusToLabelColor[instance.status];

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
              alt={`${instance.type} logo`}
              className="w-8 h-8"
            />
            <Label size="large" color={labelColor}>
              {instance.status}
            </Label>
          </div>
        </div>
        <div className="flex flex-row items-center gap-4">
          <Label size="large" color={labelColor}>
            Player Count {instance.playerCount}/{instance.maxPlayerCount}
          </Label>
          <Label
            size="large"
            color="gray"
            className="flex flex-row items-center gap-3"
          >
            <ClipboardTextfield
              text={`${instance.ip}:${instance.port}`}
              textToCopy={instance.ip}
            />
          </Label>
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
