import type React from 'react';
import { Outlet } from 'react-router';
import { isTauriMacOS } from 'utils/tauriUtil';
import { asset } from 'utils/basePath';

const LoginLayout = () => {
  return (
    <div
      // the background follows the theme in CSS, so it is right from the first paint
      className="flex h-screen flex-col justify-center bg-[image:var(--login-bg)] bg-cover bg-center p-16 lg:p-32 light:bg-[image:var(--login-bg-light)]"
      style={
        {
          '--login-bg': `url(${asset('/login_background.svg')})`,
          '--login-bg-light': `url(${asset('/login_background_light.svg')})`,
        } as React.CSSProperties
      }
    >
      {isTauriMacOS && (
        // The macOS app has no native title bar; this strip lets the window be dragged.
        <div
          data-tauri-drag-region
          className="fixed inset-x-0 top-0 z-50 h-8"
        />
      )}
      <Outlet />
    </div>
  );
};

export default LoginLayout;
