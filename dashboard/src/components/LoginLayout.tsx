import { Outlet } from 'react-router';
import { isTauriMacOS } from 'utils/tauriUtil';

const LoginLayout = () => {
  return (
    <div
      className="flex h-screen flex-col justify-center p-16 lg:p-32"
      style={{
        background: "url('/login_background.svg')",
        backgroundSize: 'cover',
        backgroundPosition: 'center',
      }}
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
