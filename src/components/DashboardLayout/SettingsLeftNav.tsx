import { useContext } from 'react';
import Button from 'components/Atoms/Button';
import { faXmark } from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';

export default function SettingsLeftNav({ className }: { className?: string }) {
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  return (
    <div className={`flex w-full flex-col items-center px-4 ${className}`}>
      <div className="flex h-full w-full grow flex-col items-center pt-12 pb-4">
        <Button
          label="Close Settings"
          icon={faXmark}
          onClick={() => {
            setSearchParam('user', undefined);
            setPathname('/');
          }}
        />
      </div>
    </div>
  );
}
