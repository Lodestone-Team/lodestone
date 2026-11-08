import { faUpload } from '@fortawesome/free-solid-svg-icons';
import { ModpackInfo } from 'bindings/ModpackInfo';
import { ModpackSource } from 'bindings/ModpackSource';
import { ModrinthSearchHit } from 'bindings/ModrinthSearchHit';
import { ModrinthSearchIndex } from 'bindings/ModrinthSearchIndex';
import { ModrinthVersion } from 'bindings/ModrinthVersion';
import Button from 'components/Atoms/Button';
import Checkbox from 'components/Atoms/Checkbox';
import WarningAlert from 'components/Atoms/WarningAlert';
import { ModIcon, pickFiles, VersionTypeLabel } from 'components/Mods/common';
import { ApiError } from 'utils/util';
import ModrinthSearch, {
  ModrinthPick,
  SortSelect,
} from 'components/Mods/ModrinthSearch';
import { InstanceContext, ModpackPreset } from 'data/InstanceContext';
import {
  createModpackInstance,
  inspectModpack,
  uploadModpack,
  useModrinthProject,
  useModrinthVersions,
} from 'data/Mods';
import { Form, Formik, FormikHelpers } from 'formik';
import { useContext, useLayoutEffect, useRef, useState } from 'react';
import { toast } from 'react-toastify';
import useAnalyticsEventTracker, { useEffectOnce } from 'utils/hooks';
import { formatDate, primaryFile } from 'utils/mods';
import { cn, errorToString, formatBytes } from 'utils/util';
import * as yup from 'yup';
import FormInputField from './SetupFormFields/FormInputField';
import FormRadioField from './SetupFormFields/FormRadioField';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';

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
      <div className="line-clamp-2 text-medium text-white/50">
        {info.summary}
      </div>
    )}
    {info.unsupported_reason && (
      <WarningAlert>
        This modpack can&apos;t be used: {info.unsupported_reason}
      </WarningAlert>
    )}
  </div>
);

/** A version row's shape while the versions load, so nothing jumps. */
const VersionRowSkeleton = () => (
  <div className="flex min-h-14 flex-row items-center gap-3 bg-gray-800 px-4 py-2">
    <div className="flex grow flex-col gap-2">
      <Skeleton className="h-3.5 w-1/4 rounded-sm" />
      <Skeleton className="h-3 w-1/2 rounded-sm" />
    </div>
    <Skeleton className="h-6 w-14 rounded-sm" />
  </div>
);

const VersionPicker = ({
  versions,
  error,
  selectedId,
  inspectingId,
  onSelect,
}: {
  versions: ModrinthVersion[] | undefined;
  error: ApiError | null;
  selectedId: string | null;
  inspectingId: string | null;
  onSelect: (version: ModrinthVersion) => void;
}) => {
  const [shown, setShown] = useState(VERSIONS_PAGE_SIZE);
  if (error)
    return (
      <div className="text-medium text-red-200">
        Failed to load versions: {errorToString(error)}
      </div>
    );
  if (!versions)
    return (
      <SkeletonGroup label="Loading versions" className={sectionClass}>
        {Array.from({ length: 6 }, (_, i) => (
          <VersionRowSkeleton key={i} />
        ))}
      </SkeletonGroup>
    );
  if (versions.length === 0)
    return (
      <div className="text-medium text-white/50">
        This modpack has no versions.
      </div>
    );
  return (
    <div className="flex flex-col gap-2">
      <div className={sectionClass}>
        {versions.slice(0, shown).map((version) => {
          const file = primaryFile(version);
          return (
            <div
              key={version.id}
              className={cn(
                'flex min-h-14 flex-row items-center gap-3 bg-gray-800 px-4 py-2',
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
                  {file && ` · ${formatBytes(file.size)}`}
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
          );
        })}
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
 * The modpack's icon, title and summary. Shown from the search result that
 * was clicked until the project loads, and as a placeholder of the same size
 * when neither is there yet (a pasted link, or a preset).
 */
const ProjectHeader = ({
  title,
  description,
  iconUrl,
}: {
  title: string | undefined;
  description: string | undefined;
  iconUrl: string | null | undefined;
}) => (
  <div className="flex h-10 flex-row items-center gap-3">
    <ModIcon url={iconUrl} />
    {title === undefined ? (
      <SkeletonGroup
        label="Loading modpack"
        className="flex grow flex-col gap-2"
      >
        <Skeleton className="h-4 w-1/3 rounded-sm" />
        <Skeleton className="h-3 w-2/3 rounded-sm" />
      </SkeletonGroup>
    ) : (
      <div className="flex min-w-0 flex-col">
        <span className="truncate text-h3 font-bold text-gray-300">
          {title}
        </span>
        <span className="truncate text-small text-white/50">{description}</span>
      </div>
    )}
  </div>
);

/** The first port from 25565 that no existing instance uses. */
const firstFreePort = (usedPorts: number[]) => {
  let port = 25565;
  while (usedPorts.includes(port)) port++;
  return port;
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
  const { instanceList } = useContext(InstanceContext);
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
  // the search result the pack was picked from, shown until the project loads
  const [pickedHit, setPickedHit] = useState<ModrinthSearchHit | null>(null);
  const { data: project } = useModrinthProject(projectId);
  const versionsQuery = useModrinthVersions(projectId, {});

  // The search step stays mounted, so Back finds its text, page and results
  // as they were; its scroll position is restored by hand.
  const sourceScroll = useRef<HTMLDivElement>(null);
  const savedScrollTop = useRef(0);
  const leaveSource = (next: Step) => {
    savedScrollTop.current = sourceScroll.current?.scrollTop ?? 0;
    setStep(next);
  };
  useLayoutEffect(() => {
    if (step === 'source' && sourceScroll.current)
      sourceScroll.current.scrollTop = savedScrollTop.current;
  }, [step]);

  // Only the latest inspect counts: an older one finishing late (after Back,
  // or another pick) must not overwrite the current choice.
  const inspectRequest = useRef(0);
  const inspect = async (versionId: string) => {
    const request = ++inspectRequest.current;
    setInspectingId(versionId);
    try {
      const result = await inspectModpack(versionId);
      if (request !== inspectRequest.current) return;
      setInfo(result);
      setSource({ type: 'modrinth', version_id: versionId });
    } catch (error) {
      if (request !== inspectRequest.current) return;
      toast.error(`Failed to read the modpack: ${errorToString(error)}`);
    } finally {
      if (request === inspectRequest.current) setInspectingId(null);
    }
  };
  const cancelInspect = () => {
    inspectRequest.current++;
    setInspectingId(null);
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
    setPickedHit(picked.hit ?? null);
    setInfo(null);
    setSource(null);
    leaveSource('version');
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
      setPickedHit(null);
      setInfo(result);
      setSource({ type: 'upload', upload_id: result.upload_id });
      leaveSource('version');
    } catch (error) {
      toast.error(`Failed to upload the modpack: ${errorToString(error)}`);
    } finally {
      setUploading(false);
    }
  };

  const back = () => {
    if (step === 'settings') setStep('version');
    else if (step === 'version' && !preset) {
      cancelInspect();
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
  const inspectingFile = (() => {
    const version = versionsQuery.data?.find((v) => v.id === inspectingId);
    return version ? primaryFile(version) : null;
  })();
  const scrollAreaClass = 'flex min-h-0 grow flex-col overflow-y-auto p-2';

  const initialValues: SettingsValues = {
    name: info?.name ?? '',
    description: info?.summary ?? '',
    port: firstFreePort(Object.values(instanceList).map((i) => i.port)),
    min_ram: 1024,
    max_ram: 4096,
    auto_start: false,
    restart_on_crash: false,
  };

  return (
    // A fixed height: the dialog is centered, so a height that followed the
    // content would move the whole dialog on every step and loading state.
    <div className="flex h-175 max-h-[calc(100vh-2rem)] w-203 rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg">
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
        <div className="flex min-h-0 grow flex-col text-left">
          {/* not needed when a preset skips it (Back then leaves the flow) */}
          {!preset && (
            <div
              ref={sourceScroll}
              className={cn(scrollAreaClass, step !== 'source' && 'hidden')}
            >
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
            </div>
          )}
          {step === 'version' && (
            <div key="version" className="flex min-h-0 grow flex-col p-2">
              <StepTitle
                title="Select a version"
                description={
                  source?.type === 'upload'
                    ? 'The uploaded modpack.'
                    : 'Pick the modpack version to install.'
                }
              />
              {projectId && (
                <ProjectHeader
                  title={project?.title ?? pickedHit?.title}
                  description={project?.description ?? pickedHit?.description}
                  iconUrl={project?.icon_url ?? pickedHit?.icon_url}
                />
              )}
              {projectId && (
                <div className="mt-4 min-h-0 grow overflow-y-auto">
                  <VersionPicker
                    versions={versionsQuery.data}
                    error={versionsQuery.error}
                    selectedId={
                      source?.type === 'modrinth' ? source.version_id : null
                    }
                    inspectingId={inspectingId}
                    onSelect={(version) => void inspect(version.id)}
                  />
                </div>
              )}
              {/* fixed below the list, with room for the summary, so picking
                  a version doesn't push the list around */}
              <div className="mt-4 flex min-h-38 shrink-0 flex-col justify-center">
                {inspectingId !== null ? (
                  <div className="text-medium text-white/50">
                    Downloading the modpack file
                    {inspectingFile
                      ? ` (${formatBytes(inspectingFile.size)})`
                      : ''}{' '}
                    to read it...
                  </div>
                ) : info ? (
                  <ModpackSummary info={info} />
                ) : (
                  <div className="text-medium text-white/50">
                    Select a version to see what it contains. Lodestone
                    downloads its modpack file to read it; nothing is installed
                    until you click Create Instance.
                  </div>
                )}
              </div>
            </div>
          )}
          {step === 'settings' && (
            <div key="settings" className={scrollAreaClass}>
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
            </div>
          )}
        </div>
        <div className="flex flex-row justify-between pt-6">
          <Button onClick={back} label="Back" disabled={submitting} />
          {/* Separate keys, so React mounts a new <button> instead of turning the
              clicked Next button into the submit button during the same click,
              which would submit the settings form before it is shown. */}
          {step === 'settings' ? (
            <Button
              key="create"
              type="submit"
              form={SETTINGS_FORM_ID}
              label="Create Instance"
              intention="primary"
              loading={submitting}
              disabled={!canContinue}
            />
          ) : step === 'version' ? (
            <Button
              key="next"
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
