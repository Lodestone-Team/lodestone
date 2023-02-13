import { useQueryClient } from '@tanstack/react-query';
import { ClientFile } from 'bindings/ClientFile';
import { FileType } from 'bindings/FileType';
import Button from 'components/Atoms/Button';
import InputField from 'components/Atoms/Form/InputField';
import { InstanceContext } from 'data/InstanceContext';
import { Form, Formik, FormikHelpers } from 'formik';
import { useContext } from 'react';
import { createInstanceDirectory } from 'utils/apis';
import { DISABLE_AUTOFILL, fileSorter } from 'utils/util';
import * as yup from 'yup';

export default function CreateFolderForm({
  onSuccess,
  onCancel,
  fileList,
  path,
}: {
  onSuccess: () => void;
  onCancel: () => void;
  fileList?: ClientFile[];
  path: string;
}) {
  const { selectedInstance: instance } = useContext(InstanceContext);
  const queryClient = useQueryClient();

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
        const error = await createInstanceDirectory(
          instance.uuid,
          path,
          values.name
        );
        if (error) {
          actions.setErrors({ name: error });
          actions.setSubmitting(false);
        } else {
          queryClient.setQueryData(
            ['instance', instance.uuid, 'fileList', path],
            fileList
              ? [
                  ...fileList,
                  {
                    name: values.name,
                    path: `${path}/${values.name}`,
                    file_type: 'Directory' as FileType,
                    creation_time: Date.now() / 1000,
                    modification_time: Date.now() / 1000,
                  },
                ].sort(fileSorter)
              : undefined
          );
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
            label="Name your folder"
            placeholder="Untitled folder"
          />
          <div className="flex flex-row justify-between">
            <Button onClick={onCancel} label="Cancel" />
            <Button
              type="submit"
              label="Create folder"
              loading={isSubmitting}
            />
          </div>
        </Form>
      )}
    </Formik>
  );
}
