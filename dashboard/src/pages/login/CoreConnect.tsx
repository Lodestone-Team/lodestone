import axios from 'axios';
import Button from 'components/Atoms/Button';
import { useContext } from 'react';
import {
  coreApiBaseUrl,
  DISABLE_AUTOFILL,
  errorToString,
  isAxiosError,
  LODESTONE_PORT,
} from 'utils/util';
import { CoreConnectionInfo, LodestoneContext } from 'data/LodestoneContext';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers, useFormikContext } from 'formik';
import * as yup from 'yup';
import SelectField from 'components/Atoms/Form/SelectField';
import {
  faArrowLeft,
  faArrowRight,
  faDownload,
} from '@fortawesome/free-solid-svg-icons';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { CoreInfo } from 'data/SystemInfo';
import { useDocumentTitle } from 'usehooks-ts';
import WarningAlert from 'components/Atoms/WarningAlert';
import { useEffect, useRef, useState } from 'react';
import ConfirmDialog from 'components/Atoms/ConfirmDialog';
import { isIPv6 } from 'is-ip';
import CoreConnectionHelp, {
  defaultProtocolFor,
  isRemoteNetworkError,
} from 'components/CoreConnectionHelp';
import Logo from 'components/Atoms/Logo';
import { t as translate, useI18n } from 'i18n';

const validationSchema = yup.object({
  address: yup.string().required(() => translate('common.required')),
  port: yup.number().required(() => translate('common.required')),
  apiVersion: yup.string().required(() => translate('common.required')),
  protocol: yup.string().required(() => translate('common.required')),
});

/** Query parameters a link can use to fill in this form. */
export const CORE_QUERY_PARAMS = [
  'address',
  'port',
  'protocol',
  'apiVersion',
] as const;

// An https page can't call an http core, browsers block it as mixed content.
const pageIsHttps = () =>
  typeof window !== 'undefined' && window.location.protocol === 'https:';

/**
 * Keeps the protocol at http for a core on this machine and https for any
 * other while the address is typed, until the user picks a protocol.
 */
const ProtocolFollowsAddress = ({ enabled }: { enabled: boolean }) => {
  const { values, setFieldValue } = useFormikContext<CoreConnectionInfo>();
  // the protocol this component last chose, null once the user chose one
  const chosenProtocol = useRef<string | null>(
    enabled ? values.protocol : null
  );
  useEffect(() => {
    if (chosenProtocol.current === null) return;
    if (values.protocol !== chosenProtocol.current) {
      chosenProtocol.current = null;
      return;
    }
    const protocol = defaultProtocolFor(values.address.trim());
    if (protocol !== values.protocol) {
      chosenProtocol.current = protocol;
      setFieldValue('protocol', protocol, false);
    }
  }, [values.address, values.protocol, setFieldValue]);
  return null;
};

const CoreConnect = () => {
  const { t, rich } = useI18n();
  useDocumentTitle(
    t('common.pageTitle', { page: t('shell.coreConnect.pageTitle') })
  );
  const { navigateBack, setPathname, searchParams, setSearchParam } =
    useContext(BrowserLocationContext);
  const { setCore, addCore } = useContext(LodestoneContext);
  const [showMixedContentError, setShowMixedContentError] = useState(false);

  // A link may suggest a core (?address=&port=&protocol=&apiVersion=). It only
  // fills in the form; the user still has to press "Add and Continue".
  const [linkProtocol] = useState(() => {
    const protocol = searchParams.get('protocol');
    return protocol === 'https' || protocol === 'http' ? protocol : null;
  });
  const [initialValues] = useState<CoreConnectionInfo>(() => {
    const apiVersion = searchParams.get('apiVersion');
    const port = searchParams.get('port');
    const address = searchParams.get('address') ?? '';
    return {
      address,
      port: port && /^\d+$/.test(port) ? port : LODESTONE_PORT.toString(),
      apiVersion: apiVersion === 'v1' ? apiVersion : 'v1',
      protocol: linkProtocol ?? defaultProtocolFor(address),
    };
  });

  // the values are in the form now, take them out of the URL
  useEffect(() => {
    CORE_QUERY_PARAMS.forEach((key) => {
      if (searchParams.has(key)) setSearchParam(key, undefined, true);
    });
  }, [searchParams, setSearchParam]);

  const onSubmit = (
    values: CoreConnectionInfo,
    actions: FormikHelpers<CoreConnectionInfo>
  ) => {
    if (
      isIPv6(values.address) &&
      !values.address.startsWith('[') &&
      !values.address.endsWith(']')
    ) {
      values.address = `[${values.address}]`;
    }
    axios
      .get<CoreInfo>(`/info`, {
        baseURL: coreApiBaseUrl(values),
      })
      .then((res) => {
        if (res.status !== 200)
          throw new Error(t('shell.login.invalidResponse'));
        setCore(values);
        addCore(values);
        if (res.data.is_setup === false) {
          setPathname('/login/core/first_setup');
        } else {
          setPathname('/login/user/select');
        }
        actions.setSubmitting(false);
        actions.setStatus(null);
      })
      .catch((err) => {
        const errorMessages = errorToString(err);
        actions.setStatus({
          error: errorMessages,
          // a copy: the form values change as the user edits them
          helpCore: isRemoteNetworkError(err, values) ? { ...values } : null,
        });
        actions.setSubmitting(false);
        // the request to the core being added was blocked as mixed content
        if (
          isAxiosError(err) &&
          !err.response &&
          pageIsHttps() &&
          values.protocol === 'http'
        )
          setShowMixedContentError(true);
        return;
      });
  };

  return (
    <div className="@container flex w-[768px] max-w-full flex-col items-stretch justify-center gap-12 rounded-2xl px-12 py-14">
      {showMixedContentError && (
        <ConfirmDialog
          isOpen={showMixedContentError}
          onClose={() => setShowMixedContentError(false)}
          title={t('shell.coreConnect.httpErrorTitle')}
          type="danger"
        >
          {rich('shell.coreConnect.mixedContent', {
            link: (
              <a
                href="https://github.com/Lodestone-Team/lodestone/wiki/Known-Issues#network-errors"
                target="_blank"
                rel="noreferrer"
                className="text-blue-200 underline hover:text-blue-300"
              >
                {t('common.learnMore')}
              </a>
            ),
          })}
        </ConfirmDialog>
      )}
      <div className="flex flex-col items-start">
        <Logo alt="Lodestone" className="h-8" />
        <h1 className="font-title text-h1 font-bold tracking-medium text-gray-300">
          {t('shell.coreConnect.title')}
        </h1>
        {pageIsHttps() && (
          <WarningAlert>
            <p>
              {rich('shell.coreConnect.httpsWarning', {
                link: (
                  <a
                    href="https://github.com/Lodestone-Team/lodestone/wiki/Known-Issues#network-errors"
                    target="_blank"
                    rel="noreferrer"
                    className="text-blue-200 underline hover:text-blue-300"
                  >
                    {t('common.learnMore')}
                  </a>
                ),
              })}
            </p>
          </WarningAlert>
        )}
      </div>
      <Formik
        initialValues={initialValues}
        validationSchema={validationSchema}
        onSubmit={onSubmit}
        validateOnBlur={false}
        validateOnChange={false}
      >
        {({ isSubmitting, status }) => (
          <Form
            id="addCoreForm"
            className="flex flex-col gap-12"
            autoComplete={DISABLE_AUTOFILL}
          >
            <ProtocolFollowsAddress enabled={linkProtocol === null} />
            {status && (
              <div className="flex flex-col gap-4">
                <WarningAlert>
                  <p>
                    {rich('shell.login.checkFields', {
                      error: <b>{status.error}</b>,
                    })}
                  </p>
                </WarningAlert>
                {status.helpCore && (
                  <CoreConnectionHelp core={status.helpCore} />
                )}
              </div>
            )}
            <div className="grid grid-cols-1 gap-x-8 gap-y-14 @lg:grid-cols-2">
              <SelectField
                name="apiVersion"
                className="grow"
                options={['v1']}
                label={t('shell.coreConnect.apiVersion')}
              />
              <SelectField
                name="protocol"
                className="grow"
                options={['http', 'https']}
                label={t('shell.coreConnect.protocol')}
              />
              <InputField
                type="number"
                name="port"
                label={t('shell.coreConnect.port')}
              />
              <InputField
                type="text"
                name="address"
                label={t('shell.coreConnect.address')}
                placeholder={t('shell.coreConnect.addressPlaceholder')}
              />
            </div>
            <div className="flex w-full flex-row flex-wrap justify-between gap-4">
              <Button
                type="button"
                icon={faArrowLeft}
                label={t('common.back')}
                onClick={navigateBack}
              />
              <div className="flex flex-row flex-wrap gap-4">
                <Button
                  type="button"
                  iconRight={faDownload}
                  label={t('shell.login.downloadCore')}
                  onClick={() => {
                    window.open(
                      'https://github.com/Lodestone-Team/dashboard/releases/',
                      '_self'
                    );
                  }}
                />
                <Button
                  type="submit"
                  intention="primary"
                  label={t('shell.coreConnect.submit')}
                  iconRight={faArrowRight}
                  loading={isSubmitting} //TODO: fix button size changing when loading
                />
              </div>
            </div>
          </Form>
        )}
      </Formik>
    </div>
  );
};

export default CoreConnect;
