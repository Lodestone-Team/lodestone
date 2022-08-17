import type { NextPage } from 'next';
import Head from 'next/head';
import Image from 'next/image';
import { useRouter } from 'next/router';
import { useEffect } from 'react';

const Dashboard: NextPage = () => {
  const router = useRouter();
  const { uuid } = router.query;

  // if no uuid, redirect to /
  useEffect(() => {
    if(!router.isReady) return;
    if (!uuid) {
      router.push(
        {
          pathname: '/',
          query: router.query,
        },
        undefined,
        { shallow: true }
      );
    }
  }, [uuid, router]);

  return (
    <div className="mx-8 my-10">
      <h1 className="font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
        {uuid}
      </h1>
    </div>
  );
};

export default Dashboard;
