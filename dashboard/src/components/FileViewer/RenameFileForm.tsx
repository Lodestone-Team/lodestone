import { useQueryClient } from '@tanstack/react-query';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { InstanceContext } from 'data/InstanceContext';
import { Form, Formik, FormikHelpers } from 'formik';
import { useContext } from 'react';
import { moveInstanceFileOrDirectory } from 'utils/apis';
import { DISABLE_AUTOFILL } from 'utils/util';
import * as yup from 'yup';
import { useFileViewer } from './FileViewerContext';

export default function CreateFileForm({
  onSuccess,
  onCancel,
  path,
}: {
  onSuccess: () => void;
  onCancel: () => void;
  path: string;
}) {
  const { selectedInstance: instance } = useContext(InstanceContext);
  const queryClient = useQueryClient();

  // the same separator as the rest of the file viewer, taken from the
  // instance path (a top-level file's own path has no separator in it)
  const { directorySeparator } = useFileViewer();

  if (!instance) throw new Error('No instance selected');

  return (
    <Formik
      initialValues={{ name: '' }}
      validationSchema={yup.object({
        name: yup.string().required('Required'),
      })}
      onSubmit={async (
        values: { name: string },
        actions: FormikHelpers<{ name: string }>
      ) => {
        actions.setSubmitting(true);

        const error = await moveInstanceFileOrDirectory(
          instance.uuid,
          path,
          path.substring(0, path.lastIndexOf(directorySeparator) + 1) +
            values.name,
          queryClient,
          directorySeparator
        );
        if (error) {
          actions.setErrors({ name: error });
          actions.setSubmitting(false);
        } else {
          actions.setSubmitting(false);
          actions.resetForm();
          onSuccess();
        }
      }}
    >
      {({ isSubmitting }) => (
        <Form
          id="create-file-form"
          autoComplete={DISABLE_AUTOFILL}
          className="flex flex-col items-stretch gap-8 text-center"
        >
          <InputField
            name="name"
            label="Name your file"
            placeholder="Untitled"
          />
          <div className="flex flex-row justify-between">
            <Button onClick={onCancel} label="Cancel" />
            <Button type="submit" label="Rename file" loading={isSubmitting} />
          </div>
        </Form>
      )}
    </Formik>
  );
}
