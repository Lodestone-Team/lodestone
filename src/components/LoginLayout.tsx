import { Outlet } from 'react-router-dom';

const LoginLayout = () => {
  return (
    <div
      className="flex h-full flex-col items-center justify-center p-8"
      style={{
        background: "url('/login_background.svg')",
        backgroundSize: 'cover',
      }}
    >
      <Outlet />
    </div>
  );
};

export default LoginLayout;
