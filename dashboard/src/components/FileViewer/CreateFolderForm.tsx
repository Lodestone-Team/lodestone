import { useQueryClient } from '@tanstack/react-query';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { isNameTaken } from 'data/FileSystem';
import { InstanceContext } from 'data/InstanceContext';
import { Form, Formik, FormikHelpers } from 'formik';
import { useContext } from 'react';
import { createInstanceDirectory } from 'utils/apis';
import { DISABLE_AUTOFILL } from 'utils/util';
import * as yup from 'yup';
import { useFileViewer } from './FileViewerContext';
import { useT } from 'i18n';

export default function CreateFolderForm({
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
        // something of that name is already there: say so, like New file does
        if (
          await isNameTaken(
            queryClient,
            instance.uuid,
            path,
            values.name,
            directorySeparator
          )
        ) {
          actions.setErrors({
            name: t('files.nameTaken', { name: values.name }),
          });
          actions.setSubmitting(false);
          return;
        }
        const error = await createInstanceDirectory(
          instance.uuid,
          path,
          values.name,
          directorySeparator
        );
        if (error) {
          actions.setErrors({ name: error });
          actions.setSubmitting(false);
        } else {
          // refetch the folder it was created in, rather than guessing the
          // new entry's metadata
          queryClient.invalidateQueries({
            queryKey: ['instance', instance.uuid, 'fileList', path],
          });
          actions.setSubmitting(false);
          actions.resetForm();
          onSuccess();
        }
      }}
    >
      {({ isSubmitting }) => (
        <Form
          id="create-folder-form"
          autoComplete={DISABLE_AUTOFILL}
          className="flex flex-col items-stretch gap-8 text-center"
        >
          <InputField
            name="name"
            label={t('files.createFolder.label')}
            placeholder={t('files.createFolder.placeholder')}
          />
          <div className="flex flex-row justify-between">
            <Button onClick={onCancel} label={t('common.cancel')} />
            <Button
              type="submit"
              label={t('files.createFolder.submit')}
              loading={isSubmitting}
            />
          </div>
        </Form>
      )}
    </Formik>
  );
}
