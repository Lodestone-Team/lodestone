import { useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import useAnalyticsEventTracker from 'utils/hooks';
import { DISABLE_AUTOFILL, errorToString } from 'utils/util';
import { createNewUser } from 'utils/apis';
import * as yup from 'yup';
import WarningAlert from 'components/Atoms/WarningAlert';
import { t as translate, useT } from 'i18n';

export type CreateNewUserValues = {
  username: string;
  password: string;
  password_confirm: string;
};

const validationSchema = yup.object({
  username: yup
    .string()
    .required(() => translate('settings.form.usernameRequired')),
  password: yup
    .string()
    .required(() => translate('settings.form.passwordRequired')),
  password_confirm: yup
    .string()
    .required(() => translate('settings.form.confirmRequired'))
    .oneOf([yup.ref('password')], () =>
      translate('settings.form.passwordsMustMatch')
    ),
});

export const CreateUserForm = ({
  onSuccess,
  onCancel,
}: {
  onSuccess: (values: CreateNewUserValues) => void;
  onCancel: () => void;
}) => {
  const t = useT();
  const gaEventTracker = useAnalyticsEventTracker('User Management');
  const queryClient = useQueryClient();
  const initialValues: CreateNewUserValues = {
    username: '',
    password: '',
    password_confirm: '',
  };

  const onSubmit = (
    values: CreateNewUserValues,
    actions: FormikHelpers<CreateNewUserValues>
  ) => {
    createNewUser({
      username: values.username,
      password: values.password,
    })
      .then((newUser) => {
        queryClient.setQueryData(
          ['user', 'list'],
          (oldData: { [uid: string]: PublicUser } | undefined) => {
            return {
              ...oldData,
              [newUser.uid]: newUser,
            };
          }
        );
        gaEventTracker('Create User', 'Success');
        onSuccess(values);
        actions.resetForm();
      })
      .catch((error) => {
        actions.setStatus({ error: errorToString(error) });
        gaEventTracker('Create User', 'Error');
      })
      .finally(() => {
        actions.setSubmitting(false);
      });
  };
  return (
    <Formik
      initialValues={initialValues}
      validationSchema={validationSchema}
      onSubmit={onSubmit}
    >
      {({ isSubmitting, status }) => (
        <Form
          id="create-user-form"
          autoComplete={DISABLE_AUTOFILL}
          className="mt-10 flex flex-col gap-16 text-left"
        >
          {status && (
            <WarningAlert>
              <p>{status.error}</p>
            </WarningAlert>
          )}
          <InputField name="username" label={t('settings.form.username')} />
          <InputField
            name="password"
            label={t('settings.form.password')}
            type="password"
          />
          <InputField
            name="password_confirm"
            label={t('settings.form.confirmPassword')}
            type="password"
          />
          <div className="flex flex-row justify-between gap-2">
            <Button onClick={onCancel} label={t('common.cancel')} />
            <Button
              type="submit"
              label={t('settings.form.createUser')}
              loading={isSubmitting}
            />
          </div>
        </Form>
      )}
    </Formik>
  );
};

export default CreateUserForm;
