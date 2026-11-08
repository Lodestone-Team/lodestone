import { Dialog, Transition } from '@headlessui/react';
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
  ApiKeyExpiryLabel,
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
import { Fragment } from 'react';
import { DISABLE_AUTOFILL } from 'utils/util';
import * as yup from 'yup';
import ApiKeyScopeEditor from './ApiKeyScopeEditor';

const FULL_ACCESS = 'Full access (everything you can do)';
const CUSTOM_ACCESS = 'Custom';

type CreateApiKeyValues = {
  name: string;
  expiry: ApiKeyExpiryLabel;
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
    .required('Name is required')
    .max(64, 'Name must be at most 64 characters'),
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
  const queryClient = useQueryClient();
  const { data: user } = useUserInfo();
  const { data: instanceList } = useInstanceList();
  const { data: globalSettings } = useGlobalSettings();
  const instances = Object.values(instanceList || {}).sort((a, b) =>
    a.name.localeCompare(b.name)
  );

  const onSubmit = async (
    values: CreateApiKeyValues,
    actions: FormikHelpers<CreateApiKeyValues>
  ) => {
    let scope: ApiKeyScope;
    if (values.access === CUSTOM_ACCESS) {
      if (countPermissions(values.permissions).permissions === 0) {
        actions.setStatus({ error: 'Choose at least one permission.' });
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
    <Transition
      appear
      show={isOpen}
      as={Fragment}
      enter="ease-out duration-200"
      enterFrom="opacity-0"
      enterTo="opacity-100"
      leave="ease-in duration-150"
      leaveFrom="opacity-100"
      leaveTo="opacity-0"
    >
      <Dialog onClose={onClose} className="relative z-10">
        <div className="fixed inset-0 bg-scrim" />
        <div className="fixed inset-0 overflow-y-auto">
          <div className="flex min-h-full items-center justify-center p-4">
            <Dialog.Panel className="flex w-180 max-w-full flex-col items-stretch justify-center gap-4 rounded-xl bg-gray-850 p-6">
              <Dialog.Title className="text-h2 font-extrabold tracking-tight text-gray-300">
                Create API key
              </Dialog.Title>
              <p className="text-medium font-medium tracking-medium text-white/50">
                The CLI, scripts and agents can sign in with an API key instead
                of your password. Give each key a name, a short expiry and only
                the access it needs.
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
                    <InputField name="name" label="Name" />
                    <RadioField
                      name="expiry"
                      label="Expires in"
                      options={API_KEY_EXPIRY_CHOICES.map((c) => c.label)}
                    />
                    {values.expiry === 'Never' && (
                      <div className="-mt-6">
                        <WarningAlert>
                          <p>
                            A key that never expires works until you revoke it.
                            Choose an expiry unless you really need it.
                          </p>
                        </WarningAlert>
                      </div>
                    )}
                    <RadioField
                      name="access"
                      label="Access"
                      options={[FULL_ACCESS, CUSTOM_ACCESS]}
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
                    <div className="flex flex-row justify-end gap-2">
                      <Button
                        label="Cancel"
                        onClick={onClose}
                        disabled={isSubmitting}
                      />
                      <Button
                        type="submit"
                        label="Create key"
                        intention="primary"
                        loading={isSubmitting}
                      />
                    </div>
                  </Form>
                )}
              </Formik>
            </Dialog.Panel>
          </div>
        </div>
      </Dialog>
    </Transition>
  );
}
