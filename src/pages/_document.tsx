import Document, { Html, Head, Main, NextScript } from 'next/document';

class MyDocument extends Document {
  render() {
    return (
      <Html>
        <Head>
          <link
            href="https://api.fontshare.com/css?f[]=chillax@1&f[]=satoshi@1,2&f[]=jet-brains-mono@1,2&f[]=clash-grotesk@1&display=swap"
            rel="stylesheet"
          />
        </Head>
        <body>
          <Main />
          <NextScript />
        </body>
      </Html>
    );
  }
}

export default MyDocument;
