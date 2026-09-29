import { ReactNode, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle2, FileLock, Lock, PlugZap, Save, XCircle } from 'lucide-react';
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

function Item({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
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

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="p-6 bg-surface rounded-xl shadow-md">
      <Text variant={TextVariants.title} color={TextColors.accent} className="mb-8">
        {title}
      </Text>
      <div className="space-y-8">{children}</div>
    </div>
  );
}

// Server address and key are saved together once they are complete; every
// other option is saved as soon as it changes, like the rest of the settings.
export default function ImmichSettings() {
  const { t } = useTranslation();
  const refreshImmich = useImmichStore((state) => state.refresh);
  const [saved, setSaved] = useState<ImmichConfig | null>(null);
  const [serverUrl, setServerUrl] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [status, setStatus] = useState<Status>({ kind: 'idle' });

  useEffect(() => {
    getImmichConfig()
      .then((config) => {
        setSaved(config);
        setServerUrl(config.serverUrl);
        setApiKey(config.apiKey);
      })
      .catch((err) => setStatus({ kind: 'error', message: String(err) }));
  }, []);

  if (!saved) return null;

  const store = async (config: ImmichConfig) => {
    await saveImmichConfig(config);
    const reloaded = await getImmichConfig();
    setSaved(reloaded);
    await refreshImmich();
    return reloaded;
  };

  const saveOption = (changes: Partial<ImmichConfig>) => {
    store({ ...saved, ...changes }).catch((err) => setStatus({ kind: 'error', message: String(err) }));
  };

  const saveConnection = async () => {
    setStatus({ kind: 'busy' });
    try {
      await store({ ...saved, serverUrl: serverUrl.trim(), apiKey: apiKey.trim() });
      setStatus({ kind: 'saved' });
    } catch (err) {
      setStatus({ kind: 'error', message: String(err) });
    }
  };

  const testConnection = async () => {
    setStatus({ kind: 'busy' });
    try {
      setStatus({ kind: 'connected', info: await testImmichConnection(serverUrl, apiKey) });
    } catch (err) {
      setStatus({ kind: 'error', message: String(err) });
    }
  };

  const isBusy = status.kind === 'busy';
  const hasCredentials = !!serverUrl.trim() && !!apiKey.trim();
  const connectionChanged = serverUrl.trim() !== saved.serverUrl || apiKey.trim() !== saved.apiKey;

  return (
    <div className="space-y-10">
      <Card title={t('immich.settings.connectionTitle')}>
        <Text variant={TextVariants.small} className="-mt-4">
          {t('immich.settings.description')}
        </Text>

        <Item label={t('immich.settings.serverUrl')} description={t('immich.settings.serverUrlDesc')}>
          <Input
            value={serverUrl}
            placeholder="https://photos.example.com"
            onChange={(e) => {
              setServerUrl(e.target.value);
              setStatus({ kind: 'idle' });
            }}
            bgClassName="bg-bg-primary"
          />
        </Item>

        <Item label={t('immich.settings.apiKey')} description={t('immich.settings.apiKeyDesc')}>
          <Input
            type="password"
            value={apiKey}
            onChange={(e) => {
              setApiKey(e.target.value);
              setStatus({ kind: 'idle' });
            }}
            bgClassName="bg-bg-primary"
          />
          {saved.apiKey && !connectionChanged && (
            <Text variant={TextVariants.small} className="mt-2 flex items-center gap-1.5">
              {saved.keyInCredentialStore ? <Lock size={12} /> : <FileLock size={12} />}
              {saved.keyInCredentialStore ? t('immich.settings.keyInStore') : t('immich.settings.keyInFile')}
            </Text>
          )}
        </Item>

        <div className="flex flex-wrap items-center gap-3">
          <Button className="bg-surface" onClick={testConnection} disabled={isBusy || !hasCredentials}>
            <PlugZap size={16} />
            {t('immich.settings.test')}
          </Button>
          <Button onClick={saveConnection} disabled={isBusy || !connectionChanged}>
            <Save size={16} />
            {t('immich.settings.save')}
          </Button>
          <StatusLine status={status} />
        </div>
      </Card>

      <Card title={t('immich.settings.workingTitle')}>
        <Item label={t('immich.settings.openStackedRaw')} description={t('immich.settings.openStackedRawDesc')}>
          <Switch
            id="immich-open-stacked-raw"
            label={t('immich.settings.openStackedRawSwitch')}
            checked={saved.openStackedRaw}
            onChange={(openStackedRaw) => saveOption({ openStackedRaw })}
          />
        </Item>
        <Item label={t('immich.settings.listingLimit')} description={t('immich.settings.listingLimitDesc')}>
          <Input
            type="number"
            value={String(saved.listingLimit)}
            onChange={(e) => setSaved({ ...saved, listingLimit: Number(e.target.value) })}
            onBlur={() =>
              saveOption({ listingLimit: Math.min(50000, Math.max(100, Math.round(saved.listingLimit || 2000))) })
            }
            className="max-w-32"
            bgClassName="bg-bg-primary"
          />
        </Item>
        <Item label={t('immich.settings.syncEdits')} description={t('immich.settings.syncEditsDesc')}>
          <Switch
            id="immich-sync-edits"
            label={t('immich.settings.syncEditsSwitch')}
            checked={saved.syncEdits}
            onChange={(syncEdits) => saveOption({ syncEdits })}
          />
        </Item>
        <Item label={t('immich.settings.uploadExports')} description={t('immich.settings.uploadExportsDesc')}>
          <Switch
            id="immich-upload-exports"
            label={t('immich.settings.uploadExportsSwitch')}
            checked={saved.uploadExports}
            onChange={(uploadExports) => saveOption({ uploadExports })}
          />
        </Item>
      </Card>

      <Card title={t('immich.settings.cacheTitle')}>
        <Item label={t('immich.settings.cacheLimit')} description={t('immich.settings.cacheLimitDesc')}>
          <Input
            type="number"
            value={String(saved.cacheLimitGb)}
            onChange={(e) => setSaved({ ...saved, cacheLimitGb: Number(e.target.value) })}
            onBlur={() => saveOption({ cacheLimitGb: Math.max(1, Math.round(saved.cacheLimitGb || 1)) })}
            className="max-w-32"
            bgClassName="bg-bg-primary"
          />
        </Item>
        <Item label={t('immich.settings.cacheDir')} description={t('immich.settings.cacheDirDesc')}>
          <Input
            value={saved.cacheDir ?? ''}
            placeholder={t('immich.settings.cacheDirDefault')}
            onChange={(e) => setSaved({ ...saved, cacheDir: e.target.value })}
            onBlur={() => saveOption({ cacheDir: saved.cacheDir?.trim() || null })}
            bgClassName="bg-bg-primary"
          />
        </Item>
      </Card>
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
          {t('immich.settings.connected', { user: status.info.userName, version: status.info.version })}
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
