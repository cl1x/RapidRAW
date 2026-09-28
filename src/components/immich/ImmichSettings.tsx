import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle2, PlugZap, Save, XCircle } from 'lucide-react';
import Button from '../ui/Button';
import Input from '../ui/Input';
import Switch from '../ui/Switch';
import Text from '../ui/Text';
import { TextColors, TextVariants } from '../../types/typography';
import {
  getImmichConfig,
  ImmichConfig,
  ImmichConnectionInfo,
  saveImmichConfig,
  testImmichConnection,
} from './immichApi';
import { useImmichStore } from './useImmichStore';

type Status =
  | { kind: 'idle' }
  | { kind: 'busy' }
  | { kind: 'connected'; info: ImmichConnectionInfo }
  | { kind: 'saved' }
  | { kind: 'error'; message: string };

function Field({ label, description, children }: { label: string; description?: string; children: React.ReactNode }) {
  return (
    <div>
      <Text variant={TextVariants.heading} className="block mb-2">
        {label}
      </Text>
      {children}
      {description && (
        <Text variant={TextVariants.small} className="mt-2">
          {description}
        </Text>
      )}
    </div>
  );
}

/** Settings page for the Immich connection, shown as its own category. */
export default function ImmichSettings() {
  const { t } = useTranslation();
  const refreshImmich = useImmichStore((state) => state.refresh);
  const [config, setConfig] = useState<ImmichConfig | null>(null);
  const [status, setStatus] = useState<Status>({ kind: 'idle' });

  useEffect(() => {
    getImmichConfig()
      .then(setConfig)
      .catch((err) => setStatus({ kind: 'error', message: String(err) }));
  }, []);

  if (!config) return null;

  const update = (changes: Partial<ImmichConfig>) => {
    setConfig({ ...config, ...changes });
    setStatus({ kind: 'idle' });
  };

  const testConnection = async () => {
    setStatus({ kind: 'busy' });
    try {
      const info = await testImmichConnection(config.serverUrl, config.apiKey);
      setStatus({ kind: 'connected', info });
    } catch (err) {
      setStatus({ kind: 'error', message: String(err) });
    }
  };

  const save = async () => {
    setStatus({ kind: 'busy' });
    try {
      await saveImmichConfig({
        ...config,
        serverUrl: config.serverUrl.trim(),
        apiKey: config.apiKey.trim(),
        cacheDir: config.cacheDir?.trim() || null,
      });
      await refreshImmich();
      setStatus({ kind: 'saved' });
    } catch (err) {
      setStatus({ kind: 'error', message: String(err) });
    }
  };

  const isBusy = status.kind === 'busy';
  const hasCredentials = !!config.serverUrl.trim() && !!config.apiKey.trim();

  return (
    <div className="space-y-10">
      <div className="p-6 bg-surface rounded-xl shadow-md">
        <Text variant={TextVariants.title} color={TextColors.accent} className="mb-2">
          {t('immich.settings.title')}
        </Text>
        <Text variant={TextVariants.small} className="mb-8">
          {t('immich.settings.description')}
        </Text>

        <div className="space-y-8">
          <Field label={t('immich.settings.serverUrl')} description={t('immich.settings.serverUrlDesc')}>
            <Input
              value={config.serverUrl}
              placeholder="https://photos.example.com"
              onChange={(e) => update({ serverUrl: e.target.value })}
              bgClassName="bg-bg-primary"
            />
          </Field>

          <Field label={t('immich.settings.apiKey')} description={t('immich.settings.apiKeyDesc')}>
            <Input
              type="password"
              value={config.apiKey}
              onChange={(e) => update({ apiKey: e.target.value })}
              bgClassName="bg-bg-primary"
            />
          </Field>

          <div className="flex flex-wrap items-center gap-3">
            <Button className="bg-surface" onClick={testConnection} disabled={isBusy || !hasCredentials}>
              <PlugZap size={16} />
              {t('immich.settings.test')}
            </Button>
            <Button onClick={save} disabled={isBusy}>
              <Save size={16} />
              {t('immich.settings.save')}
            </Button>
            <StatusLine status={status} />
          </div>
        </div>
      </div>

      <div className="p-6 bg-surface rounded-xl shadow-md">
        <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
          {t('immich.settings.behaviourTitle')}
        </Text>
        <div className="space-y-8">
          <Field label={t('immich.settings.preferRaw')} description={t('immich.settings.preferRawDesc')}>
            <Switch
              label={t('immich.settings.preferRaw')}
              checked={config.preferRaw}
              onChange={(preferRaw) => update({ preferRaw })}
            />
          </Field>

          <Field label={t('immich.settings.uploadExports')} description={t('immich.settings.uploadExportsDesc')}>
            <Switch
              label={t('immich.settings.uploadExports')}
              checked={config.uploadExports}
              onChange={(uploadExports) => update({ uploadExports })}
            />
          </Field>

          <Field
            label={t('immich.settings.replacePrevious')}
            description={t('immich.settings.replacePreviousDesc')}
          >
            <Switch
              label={t('immich.settings.replacePrevious')}
              checked={config.replacePreviousExport}
              disabled={!config.uploadExports}
              onChange={(replacePreviousExport) => update({ replacePreviousExport })}
            />
          </Field>

          <Field label={t('immich.settings.cacheLimit')} description={t('immich.settings.cacheLimitDesc')}>
            <Input
              type="number"
              value={String(config.cacheLimitGb)}
              onChange={(e) => update({ cacheLimitGb: Math.max(1, Math.round(Number(e.target.value) || 1)) })}
              className="max-w-32"
              bgClassName="bg-bg-primary"
            />
          </Field>

          <Field label={t('immich.settings.cacheDir')} description={t('immich.settings.cacheDirDesc')}>
            <Input
              value={config.cacheDir ?? ''}
              placeholder={t('immich.settings.cacheDirDefault')}
              onChange={(e) => update({ cacheDir: e.target.value })}
              bgClassName="bg-bg-primary"
            />
          </Field>

          <Button onClick={save} disabled={isBusy}>
            <Save size={16} />
            {t('immich.settings.save')}
          </Button>
        </div>
      </div>
    </div>
  );
}

function StatusLine({ status }: { status: Status }) {
  const { t } = useTranslation();

  switch (status.kind) {
    case 'busy':
      return <Text variant={TextVariants.small}>{t('immich.settings.working')}</Text>;
    case 'connected':
      return (
        <Text variant={TextVariants.small} className="flex items-center gap-1.5">
          <CheckCircle2 size={14} className="text-green-500" />
          {t('immich.settings.connected', {
            user: status.info.userName,
            version: status.info.version,
          })}
        </Text>
      );
    case 'saved':
      return (
        <Text variant={TextVariants.small} className="flex items-center gap-1.5">
          <CheckCircle2 size={14} className="text-green-500" />
          {t('immich.settings.saved')}
        </Text>
      );
    case 'error':
      return (
        <Text variant={TextVariants.small} className="flex items-center gap-1.5 break-all">
          <XCircle size={14} className="text-red-500 shrink-0" />
          {status.message}
        </Text>
      );
    default:
      return null;
  }
}
