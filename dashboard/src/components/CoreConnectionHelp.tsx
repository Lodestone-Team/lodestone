import WarningAlert from 'components/Atoms/WarningAlert';
import { CoreConnectionInfo } from 'data/LodestoneContext';
import { isAxiosError, isLocalCore } from 'utils/util';
import { useI18n } from 'i18n';

/**
 * The protocol a new core connection starts with: http for a core on this
 * machine (cores always accept plain HTTP from it), https for any other.
 */
export const defaultProtocolFor = (address: string): 'http' | 'https' =>
  isLocalCore({ address, port: '', protocol: '', apiVersion: '' })
    ? 'http'
    : 'https';

/**
 * Whether a request to `core` failed without any response from a core on
 * another device. Browsers report an untrusted certificate, a refused
 * connection and a blocked request all the same way.
 */
export const isRemoteNetworkError = (
  error: unknown,
  core: CoreConnectionInfo
): boolean => isAxiosError(error) && !error.response && !isLocalCore(core);

/**
 * Help shown after connecting to a core on another device failed with a
 * network error.
 */
export default function CoreConnectionHelp({
  core,
  savedCore = false,
}: {
  core: CoreConnectionInfo;
  /** The core was picked from the saved list, so its protocol can't be edited. */
  savedCore?: boolean;
}) {
  const { t, rich } = useI18n();
  if (core.protocol === 'https') {
    const certificatePage = `https://${core.address}:${core.port}/`;
    return (
      <WarningAlert>
        <p>
          {rich('shell.connectionHelp.trustCertificate', {
            link: (
              <a
                href={certificatePage}
                target="_blank"
                rel="noreferrer"
                className="wrap-break-word text-blue-200 underline hover:text-blue-300"
              >
                {certificatePage}
              </a>
            ),
          })}
        </p>
        <p className="mt-2">{t('shell.connectionHelp.fingerprint')}</p>
      </WarningAlert>
    );
  }
  return (
    <WarningAlert>
      <p>
        {t('shell.connectionHelp.plainHttp')}{' '}
        {savedCore
          ? t('shell.connectionHelp.savedCore')
          : t('shell.connectionHelp.changeProtocol')}
      </p>
    </WarningAlert>
  );
}
