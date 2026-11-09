import { useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import { DISABLE_AUTOFILL, errorToString } from 'utils/util';
import { changePassword } from 'utils/apis';
import * as yup from 'yup';
import WarningAlert from 'components/Atoms/WarningAlert';
import { t as translate, useT } from 'i18n';

export type ChangeUserPasswordValues = {
  password: string;
  password_confirm: string;
};

const validationSchema = yup.object({
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

export const ChangeUserPasswordForm = ({
  uid,
  onSuccess,
  onCancel,
}: {
  uid: string;
  onSuccess: () => void;
  onCancel: () => void;
}) => {
  const t = useT();
  const initialValues: ChangeUserPasswordValues = {
    password: '',
    password_confirm: '',
  };

  const onSubmit = (
    values: ChangeUserPasswordValues,
    actions: FormikHelpers<ChangeUserPasswordValues>
  ) => {
    changePassword({
      uid,
      old_password: null,
      new_password: values.password,
    })
      .then(() => {
        onSuccess();
        actions.resetForm();
      })
      .catch((error) => {
        actions.setStatus({ error: errorToString(error) });
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
          id="change-password-form"
          autoComplete={DISABLE_AUTOFILL}
          className="mt-10 flex flex-col gap-16 text-left"
        >
          {status && (
            <WarningAlert>
              <p>{status.error}</p>
            </WarningAlert>
          )}
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
              label={t('settings.form.changePassword')}
              loading={isSubmitting}
            />
          </div>
        </Form>
      )}
    </Formik>
  );
};

export default ChangeUserPasswordForm;
