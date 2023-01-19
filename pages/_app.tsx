import 'rc-tooltip/assets/bootstrap.css';
import 'globals.css';
import 'react-toastify/dist/ReactToastify.css';
import type { AppProps } from 'next/app';
import { config } from '@fortawesome/fontawesome-svg-core';
import '@fortawesome/fontawesome-svg-core/styles.css';
import axios from 'axios';
import NoSSR from 'react-no-ssr';
import { BrowserRouter } from 'react-router-dom';
import { BrowserLocationContextProvider } from 'data/BrowserLocationContext';
import { toast, ToastContainer, Zoom } from 'react-toastify';
import LoadingStatusIcon from 'components/Atoms/LoadingStatusIcon';
import ReactGA from 'react-ga4';

config.autoAddCss = false;
axios.defaults.timeout = 5000;
const TRACKING_ID = 'G-LZQ3VZ6N26';
if (process.env.NODE_ENV === 'production') ReactGA.initialize(TRACKING_ID);

const contextClass = {
  default: '!bg-gray-500',
  info: '!bg-gray-500',
  error: '!bg-red',
  warning: '!bg-yellow',
  success: '!bg-green',
};

function MyApp({ Component, pageProps }: AppProps) {
  return (
    <NoSSR>
      <ToastContainer
        toastClassName="!bg-gray-850 cursor-pointer"
        bodyClassName={() =>
          'text-medium font-medium tracking-medium font-gray-300 p-1 flex flex-row'
        }
        progressClassName={(context) => {
          const type = context?.type || 'info';
          return contextClass[type] + ' relative ' + context?.defaultClassName;
        }}
        icon={<LoadingStatusIcon level={'Info'} bright={true} />}
        position={'bottom-right'}
        closeButton={false}
        pauseOnFocusLoss={false}
        draggable={false}
        pauseOnHover
        theme="dark"
        transition={Zoom}
      />
      <BrowserRouter>
        <BrowserLocationContextProvider>
          <Component {...pageProps} />
        </BrowserLocationContextProvider>
      </BrowserRouter>
    </NoSSR>
  );
}

export default MyApp;
