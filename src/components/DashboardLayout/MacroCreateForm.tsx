import Button from 'components/Atoms/Button';
import { Field, Form, Formik, FormikHelpers, FormikProps } from 'formik';
import { useRef, useState, useEffect, useMemo } from 'react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';
import { axiosWrapper } from 'utils/util';

import { SetupInstanceManifest } from 'data/InstanceGameTypes';
import { HandlerGameType } from 'bindings/HandlerGameType';
import Spinner from 'components/DashboardLayout/Spinner';
import clsx from 'clsx';
import * as yup from 'yup';
import { ConfigurableValue } from 'components/Instance/Create/form';

export default function CreateMacro({
  onComplete,
  createNewMacro,
}: {
  onComplete: () => void;
  createNewMacro: (macroName: string, args: string[]) => void;
}) {
  const gaEventTracker = useAnalyticsEventTracker('Create Macro');
  const formikRef = useRef<FormikProps<Record<string, string>>>(null);

  useEffectOnce(() => {
    gaEventTracker('Create Macro Start');
  });

  const initialValue = {
    macro_name: '',
    args: '',
  };
  const validationSchema = yup.object().shape({
    macro_name: yup.string().required(),
    args: yup.array().of(yup.string()),
  });

  function _handleSubmit(
    values: Record<string, string>,
    actions: FormikHelpers<Record<string, string>>
  ) {
    createNewMacro(values.macro_name as string, values.args.split(','));
    actions.setSubmitting(false);
    gaEventTracker('Create Macro Success');
    onComplete();
  }

  return (
    <Formik
      initialValues={initialValue}
      validationSchema={validationSchema}
      onSubmit={_handleSubmit}
      innerRef={formikRef}
      validateOnBlur={false}
      validateOnChange={false}
    >
      {({ isSubmitting }) => (
        <Form className="flex h-fit min-h-[560px] w-[812px] rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg">
          <div className="flex w-[632px] flex-col p-9">
            <div className="flex flex-row justify-between pt-9">
              <div className="flex flex-col">
                <label className="text-medium font-medium leading-5 tracking-medium text-white/50">
                  Macro Name
                </label>
                <Field
                  name="macro_name"
                  type="text"
                  className="h-[40px] w-[300px] rounded-2xl border-2 border-gray-faded/10 bg-gray-850 text-medium font-medium leading-5 tracking-medium text-white/50"
                />
                <label className="text-medium font-medium leading-5 tracking-medium text-white/50">
                  Arguments
                </label>
                <Field
                  name="args"
                  type="text"
                  className="h-[40px] w-[300px] rounded-2xl border-2 border-gray-faded/10 bg-gray-850 text-medium font-medium leading-5 tracking-medium text-white/50"
                />
              </div>
              <Button
                type="submit"
                label={'Create Macro'}
                loading={isSubmitting}
              />
            </div>
          </div>
        </Form>
      )}
    </Formik>
  );
}
