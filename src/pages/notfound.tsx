import Button from 'components/Atoms/Button';

export default function NotFound() {
  return (
    <div className="flex h-screen w-full flex-col items-center justify-center bg-gray-900">
      <h1 className="font-title text-6xlarge font-bold">404</h1>
      <h2 className="text-2xlarge font-semibold">Page Not Found</h2>
      <Button
        label="Go Home"
        className="mt-4"
        onClick={() => {
          window.location.href = '/';
        }}
      />
    </div>
  );
}
