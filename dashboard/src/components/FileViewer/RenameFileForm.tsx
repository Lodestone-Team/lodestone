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
import { useT } from 'i18n';

export default function CreateFileForm({
  onSuccess,
  onCancel,
  path,
}: {
  onSuccess: () => void;
  onCancel: () => void;
  path: string;
}) {
  const t = useT();
  const { selectedInstance: instance } = useContext(InstanceContext);
  const queryClient = useQueryClient();

  // the same separator as the rest of the file viewer, taken from the
  // instance path (a top-level file's own path has no separator in it)
  const { directorySeparator } = useFileViewer();

  if (!instance) throw new Error('No server selected');

  return (
    <Formik
      initialValues={{ name: '' }}
      validationSchema={yup.object({
        name: yup.string().required(t('common.required')),
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
            label={t('files.rename.label')}
            placeholder={t('files.createFile.placeholder')}
          />
          <div className="flex flex-row justify-between">
            <Button onClick={onCancel} label={t('common.cancel')} />
            <Button
              type="submit"
              label={t('files.rename.submit')}
              loading={isSubmitting}
            />
          </div>
        </Form>
      )}
    </Formik>
  );
}
