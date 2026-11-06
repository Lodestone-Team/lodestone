import 'rc-tooltip/assets/bootstrap.css';
import 'globals.css';
import type { AppProps } from 'next/app';
import { config } from '@fortawesome/fontawesome-svg-core';
import axios from 'axios';
import { BrowserRouter } from 'react-router';
import { BrowserLocationContextProvider } from 'data/BrowserLocationContext';
import { ToastContainer, Zoom, IconProps } from 'react-toastify';
import LoadingStatusIcon from 'components/Atoms/LoadingStatusIcon';
import ReactGA from 'react-ga4';
import ClientOnly from 'components/ClientOnly';

// Font Awesome styles are imported (into a cascade layer) by globals.css
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
  const renderIcon = (props: IconProps) => {
    // Cannot assign 'Success' to EventLevel, so I guess there won't be a success toast
    switch (props.type) {
      case 'error':
        return <LoadingStatusIcon level={'Error'} bright={true} />;
      case 'warning':
        return <LoadingStatusIcon level={'Warning'} bright={true} />;
      default:
        return <LoadingStatusIcon level={'Info'} bright={true} />;
    }
  }

  return (
    <main>
      <ClientOnly>
        <ToastContainer
          // react-toastify 11 removed the toast body element (and bodyClassName),
          // the message is now a direct child of the toast
          toastClassName="!bg-gray-850 cursor-pointer text-medium font-medium tracking-medium"
          progressClassName={(context) => {
            const type = context?.type || 'info';
            return (
              contextClass[type] + ' relative ' + context?.defaultClassName
            );
          }}
          icon={renderIcon}
          position={'bottom-right'}
          closeButton={false}
          // the default was true before react-toastify 10
          closeOnClick
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
      </ClientOnly>
    </main>
  );
}

export default MyApp;
