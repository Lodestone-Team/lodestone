import Document, { Html, Head, Main, NextScript } from 'next/document';
import { CookiesProvider } from 'react-cookie';

class MyDocument extends Document {
  render() {
    return (
      <Html>
        <Head>
          <link
            href="https://api.fontshare.com/css?f[]=chillax@1&f[]=satoshi@1,2&display=swap"
            rel="stylesheet"
          />
        </Head>
        <body>
          <CookiesProvider>
            <Main />
            <NextScript />
          </CookiesProvider>
        </body>
      </Html>
    );
  }
}

export default MyDocument;
