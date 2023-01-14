import { useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { PublicUser } from 'bindings/PublicUser';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { Form, Formik, FormikHelpers } from 'formik';
import { createNewUser, DISABLE_AUTOFILL, errorToString } from 'utils/util';
import * as yup from 'yup';

export type CreateNewUserValues = {
  username: string;
  password: string;
  password_confirm: string;
};

const validationSchema = yup.object({
  username: yup.string().required('Username is required'),
  password: yup.string().required('Password is required'),
  password_confirm: yup
    .string()
    .required('Password confirmation is required')
    .oneOf([yup.ref('password'), null], 'Passwords must match'),
});

export const CreateUserForm = ({
  onSuccess,
  onCancel,
}: {
  onSuccess: (values: CreateNewUserValues) => void;
  onCancel: () => void;
}) => {
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
      .then((loginReply) => {
        queryClient.setQueryData(
          ['user', 'list'],
          (oldData: { [uid: string]: PublicUser } | undefined) => {
            return {
              ...oldData,
              [loginReply.user.uid]: loginReply.user,
            };
          }
        );
        onSuccess(values);
        actions.resetForm();
      })
      .catch((error) => {
        // TODO: better form errors
        actions.setErrors({ username: errorToString(error) });
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
          id="create-user-form"
          autoComplete={DISABLE_AUTOFILL}
          className="mt-10 flex flex-col gap-16 text-left"
        >
          <InputField name="username" label="Username" />
          <InputField name="password" label="Password" type="password" />
          <InputField
            name="password_confirm"
            label="Confirm Password"
            type="password"
          />
          <div className="flex flex-row justify-between">
            <Button onClick={onCancel} label="Cancel" />
            <Button type="submit" label="Create User" loading={isSubmitting} />
          </div>
        </Form>
      )}
    </Formik>
  );
};

export default CreateUserForm;
