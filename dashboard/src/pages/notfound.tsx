import Button from 'components/Atoms/Button';
import { useDocumentTitle } from 'usehooks-ts';
import { useT } from 'i18n';

export default function NotFound() {
  const t = useT();
  useDocumentTitle(t('common.pageTitle', { page: '404' }));
  return (
    <div className="flex h-screen w-full flex-col items-center justify-center bg-gray-900">
      <h1 className="font-title text-6xlarge font-bold">404</h1>
      <h2 className="text-2xlarge font-bold">{t('shell.notFound.title')}</h2>
      <Button
        label={t('shell.notFound.goHome')}
        className="mt-4"
        onClick={() => {
          window.location.href = '/';
        }}
      />
    </div>
  );
}
