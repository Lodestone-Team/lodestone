import Document, { Html, Head, Main, NextScript } from 'next/document';

class MyDocument extends Document {
  render() {
    return (
      <Html>
        <Head>
          <link
            href="https://api.fontshare.com/v2/css?f[]=chillax@1&f[]=satoshi@1,2&f[]=jet-brains-mono@1,2&f[]=clash-grotesk@1&display=swap"
            rel="stylesheet"
          />
          {/* <!-- Google tag (gtag.js) --> */}
          <script
            async
            src="https://www.googletagmanager.com/gtag/js?id=G-LZQ3VZ6N26"
          />
          <script
            dangerouslySetInnerHTML={{
              __html: `window.dataLayer = window.dataLayer || [];
          function gtag(){dataLayer.push(arguments);}
          gtag('js', new Date());

          gtag('config', 'G-LZQ3VZ6N26');`,
            }}
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
