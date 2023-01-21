import Button from 'components/Atoms/Button';
import { useDocumentTitle } from 'usehooks-ts';

export default function NotFound() {
  useDocumentTitle('404 - Lodestone');
  return (
    <div className="flex h-full w-full flex-col items-center justify-center bg-gray-900">
      <h1 className="font-title text-6xlarge font-bold">404</h1>
      <h2 className="text-2xlarge font-bold">Page Not Found</h2>
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
