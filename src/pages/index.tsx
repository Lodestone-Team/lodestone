import type { NextPage } from 'next';
import Head from 'next/head';
import Image from 'next/image';
import { useRouter } from 'next/router';

const Home: NextPage = () => {

  return (
    <div className="px-8 py-10 bg-gray-800 grow">
      <h1 className="font-semibold tracking-tight text-gray-300 text-2xlarge font-heading">
        Home
      </h1>
      <p>
        Display some general information here maybe
      </p>
    </div>
  );
};

export default Home;
