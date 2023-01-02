import { useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { useUid } from 'data/UserInfo';
import { Form, Formik, FormikHelpers } from 'formik';
import {
  changePassword,
  createNewUser,
  DISABLE_AUTOFILL,
  errorToString,
} from 'utils/util';
import * as yup from 'yup';

export type ChangeSelfPasswordValues = {
  old_password: string;
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

export const ChangeSelfPasswordForm = ({
  onSuccess,
  onCancel,
}: {
  onSuccess: () => void;
  onCancel: () => void;
}) => {
  const uid = useUid();
  const initialValues: ChangeSelfPasswordValues = {
    old_password: '',
    password: '',
    password_confirm: '',
  };

  const onSubmit = (
    values: ChangeSelfPasswordValues,
    actions: FormikHelpers<ChangeSelfPasswordValues>
  ) => {
    changePassword({
      uid,
      old_password: values.old_password,
      new_password: values.password,
    })
      .then(() => {
        onSuccess();
        actions.resetForm();
      })
      .catch((error) => {
        // TODO: better form errors
        actions.setErrors({ old_password: errorToString(error) });
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
      {({ isSubmitting }) => (
        <Form
          id="change-self-password-form"
          autoComplete={DISABLE_AUTOFILL}
          className="mt-10 flex flex-col gap-16 text-left"
        >
          <InputField
            name="old_password"
            label="Old Password"
            type="password"
          />
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

export default ChangeSelfPasswordForm;
