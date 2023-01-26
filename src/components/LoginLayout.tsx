import { Outlet } from 'react-router-dom';

const LoginLayout = () => {
  return (
    <div
      className="flex h-screen flex-col justify-center p-32"
      style={{
        background: "url('/login_background.svg')",
        backgroundSize: 'cover',
        backgroundPosition: 'center',
      }}
    >
      <Outlet />
    </div>
  );
};

export default LoginLayout;
