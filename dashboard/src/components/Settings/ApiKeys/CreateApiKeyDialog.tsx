import { Dialog, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
import { useQueryClient } from '@tanstack/react-query';
import { ApiKeyScope } from 'bindings/ApiKeyScope';
import { CreatedApiKey } from 'bindings/CreatedApiKey';
import { UserPermission } from 'bindings/UserPermission';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import RadioField from 'components/Atoms/Form/RadioField';
import WarningAlert from 'components/Atoms/WarningAlert';
import {
  addOwnApiKeyToCache,
  API_KEY_EXPIRY_CHOICES,
  apiKeyErrorMessage,
  ApiKeyExpiry,
  countPermissions,
  createApiKey,
  DEFAULT_API_KEY_EXPIRY,
  emptyPermissions,
  expiresAtFor,
} from 'data/ApiKeys';
import { useGlobalSettings } from 'data/GlobalSettings';
import { useInstanceList } from 'data/InstanceList';
import { useUserInfo } from 'data/UserInfo';
import { Form, Formik, FormikHelpers } from 'formik';
import { t as translate, useT } from 'i18n';
import { Fragment } from 'react';
import { DISABLE_AUTOFILL } from 'utils/util';
import * as yup from 'yup';
import ApiKeyScopeEditor from './ApiKeyScopeEditor';

// the access choices (form values); their labels are translated at render
const FULL_ACCESS = 'full';
const CUSTOM_ACCESS = 'custom';

const NAME_MAX_LENGTH = 64;

type CreateApiKeyValues = {
  name: string;
  expiry: ApiKeyExpiry;
  access: typeof FULL_ACCESS | typeof CUSTOM_ACCESS;
  permissions: UserPermission;
};

const initialValues = (): CreateApiKeyValues => ({
  name: '',
  expiry: DEFAULT_API_KEY_EXPIRY,
  access: FULL_ACCESS,
  permissions: emptyPermissions(),
});

const validationSchema = yup.object({
  name: yup
    .string()
    .trim()
    .required(() => translate('settings.apiKeys.nameRequired'))
    .max(NAME_MAX_LENGTH, () =>
      translate('settings.apiKeys.nameTooLong', { max: NAME_MAX_LENGTH })
    ),
});

/**
 * The dialog that creates an API key for the current user. The new key goes
 * to onCreated, and nowhere else.
 */
export default function CreateApiKeyDialog({
  isOpen,
  onClose,
  onCreated,
}: {
  isOpen: boolean;
  onClose: () => void;
  onCreated: (created: CreatedApiKey) => void;
}) {
  const t = useT();
  const queryClient = useQueryClient();
  const { data: user } = useUserInfo();
  const { data: instanceList } = useInstanceList();
  const { data: globalSettings } = useGlobalSettings();
  const instances = Object.values(instanceList || {}).sort((a, b) =>
    a.name.localeCompare(b.name)
  );

  const expiryLabel = (id: string) => {
    const choice = API_KEY_EXPIRY_CHOICES.find((c) => c.id === id);
    return choice ? t(choice.labelKey) : id;
  };
  const accessLabel = (id: string) =>
    id === CUSTOM_ACCESS
      ? t('settings.apiKeys.access.custom')
      : t('settings.apiKeys.access.full');

  const onSubmit = async (
    values: CreateApiKeyValues,
    actions: FormikHelpers<CreateApiKeyValues>
  ) => {
    let scope: ApiKeyScope;
    if (values.access === CUSTOM_ACCESS) {
      if (countPermissions(values.permissions).permissions === 0) {
        actions.setStatus({ error: t('settings.apiKeys.chooseOne') });
        return;
      }
      scope = { type: 'custom', permissions: values.permissions };
    } else {
      scope = { type: 'full' };
    }
    try {
      const created = await createApiKey({
        name: values.name.trim(),
        expires_at: expiresAtFor(values.expiry),
        scope,
      });
      addOwnApiKeyToCache(queryClient, created.info);
      actions.resetForm({ values: initialValues() });
      onCreated(created);
    } catch (e) {
      actions.setStatus({ error: apiKeyErrorMessage(e) });
    }
  };

  return (
    <Transition appear show={isOpen} as={Fragment}>
      <Dialog onClose={onClose} className="relative z-60">
        <ModalBackdrop />
        <div className="fixed inset-0 overflow-y-auto">
          <div className="flex min-h-full items-center justify-center p-4">
            <ModalPanel className="flex w-180 max-w-full flex-col items-stretch justify-center gap-4 rounded-xl bg-gray-850 p-6">
              <Dialog.Title className="text-h2 font-extrabold tracking-tight text-gray-300">
                {t('settings.apiKeys.create')}
              </Dialog.Title>
              <p className="text-medium font-medium tracking-medium text-white/50">
                {t('settings.apiKeys.createLead')}
              </p>
              <Formik
                initialValues={initialValues()}
                validationSchema={validationSchema}
                onSubmit={onSubmit}
              >
                {({ isSubmitting, status, values, setFieldValue }) => (
                  <Form
                    id="create-api-key-form"
                    autoComplete={DISABLE_AUTOFILL}
                    className="mt-6 flex flex-col gap-12 text-left"
                  >
                    <InputField
                      name="name"
                      label={t('settings.apiKeys.name')}
                    />
                    <RadioField
                      name="expiry"
                      label={t('settings.apiKeys.expiresIn')}
                      options={API_KEY_EXPIRY_CHOICES.map((c) => c.id)}
                      optionLabel={expiryLabel}
                    />
                    {values.expiry === 'never' && (
                      <div className="-mt-6">
                        <WarningAlert>
                          <p>{t('settings.apiKeys.neverWarning')}</p>
                        </WarningAlert>
                      </div>
                    )}
                    <RadioField
                      name="access"
                      label={t('settings.apiKeys.access')}
                      options={[FULL_ACCESS, CUSTOM_ACCESS]}
                      optionLabel={accessLabel}
                    />
                    {values.access === CUSTOM_ACCESS && user && (
                      <div className="-mt-4 rounded-lg border border-gray-faded/30 p-4">
                        <ApiKeyScopeEditor
                          value={values.permissions}
                          onChange={(permissions) =>
                            setFieldValue('permissions', permissions)
                          }
                          user={user}
                          instances={instances}
                          safeMode={globalSettings?.safe_mode || false}
                        />
                      </div>
                    )}
                    {status?.error && (
                      <WarningAlert>
                        <p>{status.error}</p>
                      </WarningAlert>
                    )}
                    <div className="flex flex-row flex-wrap justify-end gap-2">
                      <Button
                        label={t('common.cancel')}
                        onClick={onClose}
                        disabled={isSubmitting}
                      />
                      <Button
                        type="submit"
                        label={t('settings.apiKeys.createKey')}
                        intention="primary"
                        loading={isSubmitting}
                      />
                    </div>
                  </Form>
                )}
              </Formik>
            </ModalPanel>
          </div>
        </div>
      </Dialog>
    </Transition>
  );
}
