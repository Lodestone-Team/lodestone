import { useQuery, useQueryClient } from '@tanstack/react-query';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { ResourcePackHash } from 'bindings/ResourcePackHash';
import { ServerResourcePack } from 'bindings/ServerResourcePack';
import Button from 'components/Atoms/Button';
import { Toggle } from 'components/Atoms/Toggle';
import { useState } from 'react';
import { toast } from 'react-toastify';
import { formatPackSize, isSha1, packUrlError } from 'utils/resourcePack';
import { axiosWrapper, DISABLE_AUTOFILL, errorToString } from 'utils/util';
import {
  SettingsCard,
  SettingsRow,
  SettingsSection,
} from 'components/Settings/SettingsBlocks';

const EMPTY: ServerResourcePack = {
  url: '',
  sha1: '',
  required: false,
  prompt: '',
};

const inputClass =
  'input-shape input-background input-outlines input-text-style input-border-normal w-full';

/**
 * The server resource pack: the `resource-pack`, `resource-pack-sha1`,
 * `require-resource-pack` and `resource-pack-prompt` lines of
 * server.properties. The core downloads the URL to compute the SHA-1.
 */
export function ResourcePackSetting({
  instance,
  canEdit,
  disabledReason,
}: {
  instance: InstanceInfo;
  canEdit: boolean;
  disabledReason: string;
}) {
  const queryClient = useQueryClient();
  const queryKey = ['instances', instance.uuid, 'resource_pack'];
  const { data: saved } = useQuery({
    queryKey,
    queryFn: () =>
      axiosWrapper<ServerResourcePack>({
        method: 'GET',
        url: `/instance/${instance.uuid}/resource_pack`,
      }),
    enabled: instance.game_type.type === 'MinecraftJava',
    retry: false,
  });
  // the edits; `null` shows the saved pack
  const [edited, setDraft] = useState<ServerResourcePack | null>(null);
  const [busy, setBusy] = useState<'saving' | 'hashing' | null>(null);
  const [hashNote, setHashNote] = useState<string | null>(null);
  if (!saved) return null;
  const draft = edited ?? saved;

  const urlError = packUrlError(draft.url);
  const sha1Error = isSha1(draft.sha1)
    ? null
    : 'The SHA-1 is 40 hexadecimal characters.';
  const changed =
    draft.url.trim() !== saved.url ||
    draft.sha1.trim().toLowerCase() !== saved.sha1 ||
    draft.required !== saved.required ||
    draft.prompt.trim() !== saved.prompt;
  const disabled = !canEdit || busy !== null;

  const save = async (pack: ServerResourcePack) => {
    setBusy('saving');
    try {
      const result = await axiosWrapper<ServerResourcePack>({
        method: 'PUT',
        url: `/instance/${instance.uuid}/resource_pack`,
        data: pack,
      });
      queryClient.setQueryData(queryKey, result);
      setDraft(null);
      toast.success(
        instance.state === 'Stopped'
          ? 'Resource pack saved'
          : 'Resource pack saved. Restart the server to use it.'
      );
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setBusy(null);
    }
  };

  const computeSha1 = async () => {
    setBusy('hashing');
    setHashNote(null);
    try {
      const hash = await axiosWrapper<ResourcePackHash>({
        method: 'POST',
        url: `/instance/${instance.uuid}/resource_pack/hash`,
        data: { url: draft.url.trim() },
      });
      setDraft({ ...draft, sha1: hash.sha1 });
      setHashNote(`Downloaded ${formatPackSize(hash.size)}.`);
    } catch (err) {
      toast.error(errorToString(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <SettingsSection
      id="server-settings-resource-pack"
      title="Resource pack"
      lead="A pack players are asked to download when they join."
    >
      <SettingsCard>
        <SettingsRow
          label="URL"
          htmlFor="resource-pack-url"
          hint={
            canEdit
              ? 'A direct link to the pack zip. Leave it empty for none.'
              : disabledReason
          }
          error={canEdit ? urlError : null}
          controlClassName="w-5/12"
        >
          <input
            id="resource-pack-url"
            className={inputClass}
            value={draft.url}
            placeholder="https://example.com/pack.zip"
            maxLength={2048}
            disabled={disabled}
            autoComplete={DISABLE_AUTOFILL}
            onChange={(e) => {
              setDraft({ ...draft, url: e.target.value });
              setHashNote(null);
            }}
          />
        </SettingsRow>
        <SettingsRow
          label="SHA-1"
          htmlFor="resource-pack-sha1"
          hint={
            hashNote ?? 'Lets players check the download and cache the pack.'
          }
          error={sha1Error}
          controlClassName="w-5/12"
        >
          <input
            id="resource-pack-sha1"
            className={`${inputClass} min-w-0 font-mono`}
            value={draft.sha1}
            maxLength={40}
            disabled={disabled}
            autoComplete={DISABLE_AUTOFILL}
            onChange={(e) => setDraft({ ...draft, sha1: e.target.value })}
          />
          <Button
            label="Compute"
            className="shrink-0"
            loading={busy === 'hashing'}
            disabled={disabled || !draft.url.trim() || urlError !== null}
            onClick={() => void computeSha1()}
          />
        </SettingsRow>
        <SettingsRow
          label="Require the pack"
          labelId="resource-pack-required"
          hint="Players who decline it are disconnected."
        >
          <Toggle
            value={draft.required}
            onChange={(required) => setDraft({ ...draft, required })}
            disabled={disabled}
            aria-labelledby="resource-pack-required"
          />
        </SettingsRow>
        <SettingsRow
          label="Prompt"
          htmlFor="resource-pack-prompt"
          hint="The message shown when players are asked. Plain text, or a JSON text component."
          controlClassName="w-5/12"
        >
          <input
            id="resource-pack-prompt"
            className={inputClass}
            value={draft.prompt}
            maxLength={2048}
            disabled={disabled}
            autoComplete={DISABLE_AUTOFILL}
            onChange={(e) => setDraft({ ...draft, prompt: e.target.value })}
          />
        </SettingsRow>
        {/* changes here are kept until saved, so the pack is set at once */}
        <div className="flex flex-row justify-end gap-2 px-4 py-3">
          <Button
            label="Remove pack"
            disabled={disabled || !saved.url}
            onClick={() => void save(EMPTY)}
          />
          <Button
            label="Save"
            intention="primary"
            loading={busy === 'saving'}
            disabled={
              disabled || !changed || urlError !== null || sha1Error !== null
            }
            onClick={() => void save(draft)}
          />
        </div>
      </SettingsCard>
    </SettingsSection>
  );
}
