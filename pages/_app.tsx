import '../styles/globals.css';
import type { AppProps } from 'next/app';
import DashboardLayout from '../components/DashboardLayout';

function MyApp({ Component, pageProps }: AppProps) {
  return (
    <DashboardLayout>
      <Component {...pageProps} />
    </DashboardLayout>
  );
}

export default MyApp;
