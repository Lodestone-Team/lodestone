import Document, { Html, Head, Main, NextScript } from 'next/document';
import { THEME_INIT_SCRIPT } from 'utils/theme';

class MyDocument extends Document {
  render() {
    return (
      <Html>
        <Head>
          {/* Sets data-theme and color-scheme on <html> from the saved
              preference or the OS setting, so the first paint has the right
              theme (see utils/theme.ts) */}
          <script dangerouslySetInnerHTML={{ __html: THEME_INIT_SCRIPT }} />
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
