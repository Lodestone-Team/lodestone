import DashboardLayout from 'components/DashboardLayout';
import Head from 'next/head';
import Image from 'next/image';
import { useRouter } from 'next/router';
import { ReactElement, ReactNode, useEffect } from 'react';
import { NextPageWithLayout } from './_app';

const Home: NextPageWithLayout = () => {
  const router = useRouter();
  // get rid of uuid from query
  const { uuid } = router.query;

  useEffect(() => {
    if (uuid) {
      router.push({
        pathname: `/dashboard`,
        query: {
          ...router.query,
        },
      });
    }
  }, [uuid]);

  return (
    <div className="h-0 px-8 py-10 bg-gray-800 grow">
      <h1 className="font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
        Home
      </h1>
      <p>Display some general information here maybe</p>
    </div>
  );
};

Home.getLayout = (page: ReactElement): ReactNode => (
  <DashboardLayout>{page}</DashboardLayout>
);

export default Home;
