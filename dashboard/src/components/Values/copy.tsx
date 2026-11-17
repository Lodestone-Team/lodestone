import { MonoText } from 'components/Atoms/Values';
import { t } from 'i18n';
import { toast } from 'react-toastify';
import { ObjectToast } from './ObjectToast';

/**
 * Copies text and says so: "Copied", with the value on a mono line under it
 * (cut in the middle). navigator.clipboard only exists over HTTPS and on
 * localhost.
 */
export function copyWithToast(text: string) {
  navigator.clipboard.writeText(text);
  toast.info(
    <ObjectToast outcome={t('values.copied')}>
      <MonoText
        text={text}
        max={36}
        className="text-small break-all text-gray-400"
      />
    </ObjectToast>
  );
}
