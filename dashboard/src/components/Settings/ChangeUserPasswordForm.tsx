import { useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { PublicUser } from '@bindings/PublicUser';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import { DISABLE_AUTOFILL, errorToString } from 'utils/util';
import { changePassword, createNewUser } from 'utils/apis';
import * as yup from 'yup';
import WarningAlert from 'components/Atoms/WarningAlert';

export type ChangeUserPasswordValues = {
  password: string;
  password_confirm: string;
};

const validationSchema = yup.object({
  password: yup.string().required('Password is required'),
  password_confirm: yup
    .string()
    .required('Password confirmation is required')
    .oneOf([yup.ref('password'), null], 'Passwords must match'),
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
          <InputField name="password" label="Password" type="password" />
          <InputField
            name="password_confirm"
            label="Confirm Password"
            type="password"
          />
          <div className="flex flex-row justify-between">
            <Button onClick={onCancel} label="Cancel" />
            <Button
              type="submit"
              label="Change Password"
              loading={isSubmitting}
            />
          </div>
        </Form>
      )}
    </Formik>
  );
};

export default ChangeUserPasswordForm;
