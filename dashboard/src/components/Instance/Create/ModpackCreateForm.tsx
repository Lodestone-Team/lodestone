import { faUpload } from '@fortawesome/free-solid-svg-icons';
import { ModpackInfo } from 'bindings/ModpackInfo';
import { ModpackSource } from 'bindings/ModpackSource';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import { ModrinthVersion } from 'bindings/ModrinthVersion';
import Button from 'components/Atoms/Button';
import Checkbox from 'components/Atoms/Checkbox';
import WarningAlert from 'components/Atoms/WarningAlert';
import { ModIcon, pickFiles, VersionTypeLabel } from 'components/Mods/common';
import ModrinthSearch, {
  ModrinthPick,
  SortSelect,
} from 'components/Mods/ModrinthSearch';
import { ModpackPreset } from 'data/InstanceContext';
import {
  createModpackInstance,
  inspectModpack,
  uploadModpack,
  useModrinthProject,
  useModrinthVersions,
} from 'data/Mods';
import { Form, Formik, FormikHelpers } from 'formik';
import { useState } from 'react';
import { toast } from 'react-toastify';
import useAnalyticsEventTracker, { useEffectOnce } from 'utils/hooks';
import { formatDate } from 'utils/mods';
import { cn, errorToString, formatBytes } from 'utils/util';
import * as yup from 'yup';
import FormInputField from './SetupFormFields/FormInputField';
import FormRadioField from './SetupFormFields/FormRadioField';

type Step = 'source' | 'version' | 'settings';

const STEPS: Array<[Step, string]> = [
  ['source', 'Select Modpack'],
  ['version', 'Select Version'],
  ['settings', 'Instance Settings'],
];

const VERSIONS_PAGE_SIZE = 20;
const SETTINGS_FORM_ID = 'modpackInstanceSettingsForm';

type SettingsValues = {
  name: string;
  description: string;
  port: number;
  min_ram: number;
  max_ram: number;
  auto_start: boolean;
  restart_on_crash: boolean;
};

const settingsSchema = yup.object({
  name: yup.string().trim().required('Name is required'),
  description: yup.string(),
  port: yup
    .number()
    .typeError('Port must be a number')
    .integer('Port must be an integer')
    .min(1, 'Port must be between 1 and 65535')
    .max(65535, 'Port must be between 1 and 65535')
    .required('Port is required'),
  min_ram: yup
    .number()
    .typeError('Minimum RAM must be a number')
    .integer('Minimum RAM must be an integer')
    .min(1, 'Minimum RAM must be at least 1 MB')
    .required('Minimum RAM is required'),
  max_ram: yup
    .number()
    .typeError('Maximum RAM must be a number')
    .integer('Maximum RAM must be an integer')
    .min(yup.ref('min_ram'), 'Maximum RAM must be at least the minimum RAM')
    .required('Maximum RAM is required'),
  auto_start: yup.boolean(),
  restart_on_crash: yup.boolean(),
});

const sectionClass =
  'flex flex-col rounded-md border border-gray-faded/30 text-left child:border-b child:border-gray-faded/30 child:first:rounded-t-lg child:last:rounded-b-lg child:last:border-b-0';

const StepTitle = ({
  title,
  description,
}: {
  title: string;
  description: string;
}) => (
  <>
    <div className="text-left text-h2 leading-7 font-extrabold tracking-medium text-white">
      {title}
    </div>
    <div className="mb-4 text-left text-medium leading-4 font-mediumbold text-white/50 italic">
      {description}
    </div>
  </>
);

/** What the core found in the pack, and whether Lodestone can run it. */
const ModpackSummary = ({ info }: { info: ModpackInfo }) => (
  <div className="flex flex-col gap-3">
    <div className={sectionClass}>
      {(
        [
          [
            'Modpack',
            `${info.name}${info.version_number ? ` ${info.version_number}` : ''}`,
          ],
          ['Minecraft', info.game_version],
          [
            'Loader',
            `${info.loader}${info.loader_version ? ` ${info.loader_version}` : ''}`,
          ],
          [
            'Server files',
            `${info.server_file_count} (${formatBytes(info.server_download_size)} to download)`,
          ],
        ] as Array<[string, string]>
      ).map(([label, value]) => (
        <div
          key={label}
          className="flex flex-row justify-between gap-4 bg-gray-800 px-4 py-2 text-medium"
        >
          <span className="text-white/50">{label}</span>
          <span className="truncate text-gray-300">{value}</span>
        </div>
      ))}
    </div>
    {info.summary && (
      <div className="text-medium text-white/50">{info.summary}</div>
    )}
    {info.unsupported_reason && (
      <WarningAlert>
        This modpack can&apos;t be used: {info.unsupported_reason}
      </WarningAlert>
    )}
  </div>
);

const VersionPicker = ({
  projectId,
  selectedId,
  inspectingId,
  onSelect,
}: {
  projectId: string;
  selectedId: string | null;
  inspectingId: string | null;
  onSelect: (version: ModrinthVersion) => void;
}) => {
  const [shown, setShown] = useState(VERSIONS_PAGE_SIZE);
  const {
    data: versions,
    error,
    isPending,
  } = useModrinthVersions(projectId, {});
  if (error)
    return (
      <div className="text-medium text-red-200">
        Failed to load versions: {errorToString(error)}
      </div>
    );
  if (isPending || !versions)
    return <div className="text-medium text-white/50">Loading versions...</div>;
  if (versions.length === 0)
    return (
      <div className="text-medium text-white/50">
        This modpack has no versions.
      </div>
    );
  return (
    <div className="flex flex-col gap-2">
      <div className={sectionClass}>
        {versions.slice(0, shown).map((version) => (
          <div
            key={version.id}
            className={cn(
              'flex flex-row items-center gap-3 bg-gray-800 px-4 py-2',
              version.id === selectedId && 'bg-gray-700'
            )}
          >
            <div className="flex min-w-0 grow flex-col">
              <div className="flex min-w-0 flex-row items-center gap-2">
                <span className="truncate text-medium font-bold text-gray-300">
                  {version.version_number}
                </span>
                <VersionTypeLabel type={version.version_type} />
              </div>
              <div className="truncate text-small text-white/50">
                {version.game_versions.join(', ')} ·{' '}
                {version.loaders.join(', ')} ·{' '}
                {formatDate(version.date_published)}
              </div>
            </div>
            <Button
              label={version.id === selectedId ? 'Selected' : 'Select'}
              size="slim"
              intention={version.id === selectedId ? 'primary' : 'info'}
              loading={inspectingId === version.id}
              disabled={inspectingId !== null && inspectingId !== version.id}
              onClick={() => onSelect(version)}
            />
          </div>
        ))}
      </div>
      {versions.length > shown && (
        <Button
          label={`Show more (${versions.length - shown} left)`}
          className="self-center"
          onClick={() => setShown(shown + VERSIONS_PAGE_SIZE)}
        />
      )}
    </div>
  );
};

/**
 * Creating an instance from a Modrinth modpack (.mrpack): pick a pack
 * (search, link or upload), pick a version, then the instance settings.
 * The core creates the instance in the background, with the usual
 * progression notifications.
 */
export default function ModpackCreateForm({
  preset,
  onBack,
  onComplete,
}: {
  preset: ModpackPreset | null;
  /** back to the game type selection */
  onBack: () => void;
  onComplete: () => void;
}) {
  const gaEventTracker = useAnalyticsEventTracker('Create Instance');
  const [step, setStep] = useState<Step>(preset ? 'version' : 'source');
  const [projectId, setProjectId] = useState<string | null>(
    preset?.projectId ?? null
  );
  const [onlyFabric, setOnlyFabric] = useState(true);
  const [index, setIndex] = useState<ModrinthSearchIndex>('relevance');
  const [info, setInfo] = useState<ModpackInfo | null>(null);
  const [source, setSource] = useState<ModpackSource | null>(null);
  const [inspectingId, setInspectingId] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const { data: project } = useModrinthProject(projectId);

  const inspect = async (versionId: string) => {
    setInspectingId(versionId);
    try {
      const result = await inspectModpack(versionId);
      setInfo(result);
      setSource({ type: 'modrinth', version_id: versionId });
    } catch (error) {
      toast.error(`Failed to read the modpack: ${errorToString(error)}`);
    } finally {
      setInspectingId(null);
    }
  };

  useEffectOnce(() => {
    if (preset?.versionId) void inspect(preset.versionId);
  });

  const pick = (picked: ModrinthPick) => {
    if (picked.projectType !== 'modpack') {
      toast.error(`That is a ${picked.projectType}, not a modpack`);
      return;
    }
    setProjectId(picked.projectId);
    setInfo(null);
    setSource(null);
    setStep('version');
    if (picked.versionId) void inspect(picked.versionId);
  };

  const upload = async () => {
    const [file] = await pickFiles('.mrpack', false);
    if (!file) return;
    if (!file.name.toLowerCase().endsWith('.mrpack')) {
      toast.error('Choose a .mrpack file');
      return;
    }
    setUploading(true);
    try {
      const result = await uploadModpack(file);
      if (!result.upload_id) throw new Error('The core returned no upload id');
      setProjectId(null);
      setInfo(result);
      setSource({ type: 'upload', upload_id: result.upload_id });
      setStep('version');
    } catch (error) {
      toast.error(`Failed to upload the modpack: ${errorToString(error)}`);
    } finally {
      setUploading(false);
    }
  };

  const back = () => {
    if (step === 'settings') setStep('version');
    else if (step === 'version' && !preset) {
      setStep('source');
      setInfo(null);
      setSource(null);
    } else onBack();
  };

  const submit = async (
    values: SettingsValues,
    actions: FormikHelpers<SettingsValues>
  ) => {
    if (!source) return;
    setSubmitting(true);
    try {
      await createModpackInstance({
        source,
        name: values.name.trim(),
        port: values.port,
        description: values.description,
        min_ram: values.min_ram,
        max_ram: values.max_ram,
        auto_start: values.auto_start,
        restart_on_crash: values.restart_on_crash,
      });
    } catch (error) {
      // keep the dialog open so the input isn't lost
      toast.error('Error creating instance: ' + errorToString(error));
      actions.setSubmitting(false);
      setSubmitting(false);
      return;
    }
    actions.setSubmitting(false);
    setSubmitting(false);
    gaEventTracker('Create Instance Complete');
    onComplete();
  };

  const canContinue = !!info && !!source && !info.unsupported_reason;
  const busy = inspectingId !== null || uploading;

  const initialValues: SettingsValues = {
    name: info?.name ?? '',
    description: info?.summary ?? '',
    port: 25565,
    min_ram: 1024,
    max_ram: 4096,
    auto_start: false,
    restart_on_crash: false,
  };

  return (
    <div className="flex max-h-175 min-h-140 w-203 rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg">
      <div className="w-45 shrink-0 border-r border-gray-700 pt-8">
        <div className="px-4 py-2 text-left font-sans text-medium leading-5 font-medium tracking-medium text-white/50">
          Select Game
        </div>
        {STEPS.map(([id, label]) => (
          <div
            key={id}
            className={cn(
              'px-4 py-2 text-left font-sans text-medium leading-5 font-medium tracking-medium text-white/50',
              step === id && 'font-extrabold text-white'
            )}
          >
            {label}
          </div>
        ))}
      </div>
      <div className="flex w-158 min-w-0 flex-col p-8">
        <div className="flex min-h-0 grow flex-col overflow-y-auto p-2 text-left">
          {step === 'source' && (
            <>
              <StepTitle
                title="Select a modpack"
                description="Search Modrinth, paste a Modrinth link, or upload a .mrpack file. Only Fabric modpacks are supported."
              />
              <ModrinthSearch
                projectType="modpack"
                loader={onlyFabric ? 'fabric' : undefined}
                index={index}
                placeholder="Search modpacks, or paste a Modrinth link"
                onPick={pick}
                toolbar={
                  <div className="flex flex-row flex-wrap items-center justify-between gap-3">
                    <Checkbox
                      label="Only show Fabric modpacks"
                      checked={onlyFabric}
                      onChange={setOnlyFabric}
                    />
                    <div className="flex flex-row items-center gap-2">
                      <SortSelect value={index} onChange={setIndex} />
                      <Button
                        label="Upload .mrpack"
                        icon={faUpload}
                        loading={uploading}
                        onClick={upload}
                      />
                    </div>
                  </div>
                }
              />
            </>
          )}
          {step === 'version' && (
            <>
              <StepTitle
                title="Select a version"
                description={
                  source?.type === 'upload'
                    ? 'The uploaded modpack.'
                    : 'Pick the modpack version to install.'
                }
              />
              <div className="flex flex-col gap-4">
                {projectId && (
                  <div className="flex flex-row items-center gap-3">
                    <ModIcon url={project?.icon_url} />
                    <div className="flex min-w-0 flex-col">
                      <span className="truncate text-h3 font-bold text-gray-300">
                        {project?.title ?? 'Loading...'}
                      </span>
                      <span className="truncate text-small text-white/50">
                        {project?.description}
                      </span>
                    </div>
                  </div>
                )}
                {inspectingId !== null && (
                  <div className="text-medium text-white/50">
                    Downloading and reading the modpack...
                  </div>
                )}
                {info && <ModpackSummary info={info} />}
                {projectId && (
                  <VersionPicker
                    projectId={projectId}
                    selectedId={
                      source?.type === 'modrinth' ? source.version_id : null
                    }
                    inspectingId={inspectingId}
                    onSelect={(version) => void inspect(version.id)}
                  />
                )}
              </div>
            </>
          )}
          {step === 'settings' && (
            <Formik
              initialValues={initialValues}
              validationSchema={settingsSchema}
              onSubmit={submit}
              validateOnBlur={false}
              validateOnChange={false}
            >
              <Form id={SETTINGS_FORM_ID}>
                <StepTitle
                  title="Instance settings"
                  description="Configure your server."
                />
                {info && (
                  <div className="mb-4 text-medium text-white/50">
                    {info.name} · Minecraft {info.game_version} · Fabric{' '}
                    {info.loader_version}
                  </div>
                )}
                <div className={sectionClass}>
                  <FormInputField
                    name="name"
                    label="Name"
                    type="text"
                    description="The name of the instance"
                  />
                  <FormInputField
                    name="description"
                    label="Description"
                    type="text"
                    description="The description of the instance"
                    optional
                  />
                  <FormInputField
                    name="port"
                    label="Port"
                    type="number"
                    description="The port to run the server on"
                  />
                </div>
                <div className={cn(sectionClass, 'mt-6')}>
                  <FormInputField
                    name="min_ram"
                    label="Minimum RAM"
                    type="number"
                    description="The minimum amount of RAM to allocate to the server, in MB"
                  />
                  <FormInputField
                    name="max_ram"
                    label="Maximum RAM"
                    type="number"
                    description="The maximum amount of RAM to allocate to the server, in MB"
                  />
                </div>
                <div className={cn(sectionClass, 'mt-6')}>
                  <FormRadioField
                    name="auto_start"
                    label="Auto Start"
                    options={['false', 'true']}
                    description="The instance will start automatically when the application starts"
                  />
                  <FormRadioField
                    name="restart_on_crash"
                    label="Restart on Crash"
                    options={['false', 'true']}
                    description="The instance will restart automatically if it crashes"
                  />
                </div>
              </Form>
            </Formik>
          )}
        </div>
        <div className="flex flex-row justify-between pt-6">
          <Button onClick={back} label="Back" disabled={submitting} />
          {step === 'settings' ? (
            <Button
              type="submit"
              form={SETTINGS_FORM_ID}
              label="Create Instance"
              intention="primary"
              loading={submitting}
              disabled={!canContinue}
            />
          ) : step === 'version' ? (
            <Button
              label="Next"
              disabled={!canContinue || busy}
              onClick={() => setStep('settings')}
            />
          ) : (
            <div></div>
          )}
        </div>
      </div>
    </div>
  );
}
