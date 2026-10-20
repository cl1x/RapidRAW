import { ReactNode, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Search, X } from 'lucide-react';
import Text from '../ui/Text';
import { TextVariants } from '../../types/typography';
import {
  getImmichSuggestions,
  ImmichAlbum,
  ImmichFilter,
  ImmichPerson,
  listImmichPeople,
  SuggestionKind,
} from './immichApi';

const NOT_IN_ALBUM = '__none__';

const fieldClass =
  'w-full h-8 rounded-md bg-surface px-2 text-sm text-text-primary border border-transparent focus:outline-hidden focus:border-accent';

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <Text variant={TextVariants.small} className="block mb-1">
        {label}
      </Text>
      {children}
    </label>
  );
}

function Choice({
  value,
  options,
  onChange,
  anyLabel,
}: {
  value: string | null | undefined;
  options: { value: string; label: string }[];
  onChange(value: string | null): void;
  anyLabel: string;
}) {
  return (
    <select className={fieldClass} value={value ?? ''} onChange={(e) => onChange(e.target.value || null)}>
      <option value="">{anyLabel}</option>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

const asOptions = (values: string[]) => values.map((v) => ({ value: v, label: v }));

function useSuggestions(kind: SuggestionKind, narrow: { country?: string; make?: string } = {}) {
  const [values, setValues] = useState<string[]>([]);
  const { country, make } = narrow;
  useEffect(() => {
    getImmichSuggestions(kind, { country, make })
      .then(setValues)
      .catch(() => setValues([]));
  }, [kind, country, make]);
  return values;
}

export default function ImmichFilterForm({
  albums,
  initial,
  onApply,
}: {
  albums: ImmichAlbum[];
  initial: ImmichFilter;
  onApply(filter: ImmichFilter): void;
}) {
  const { t } = useTranslation();
  const [filter, setFilter] = useState<ImmichFilter>(initial);
  const [people, setPeople] = useState<ImmichPerson[]>([]);
  const countries = useSuggestions('country');
  const cities = useSuggestions('city', { country: filter.country ?? undefined });
  const makes = useSuggestions('camera-make');
  const models = useSuggestions('camera-model', { make: filter.make ?? undefined });

  useEffect(() => {
    listImmichPeople()
      .then(setPeople)
      .catch(() => setPeople([]));
  }, []);

  const update = (changes: Partial<ImmichFilter>) => setFilter((f) => ({ ...f, ...changes }));
  const albumValue = filter.notInAlbum ? NOT_IN_ALBUM : (filter.albumId ?? '');

  return (
    <form
      className="mx-1 my-1 p-2 space-y-2 rounded-md bg-bg-primary/60"
      onSubmit={(e) => {
        e.preventDefault();
        onApply(filter);
      }}
    >
      <Row label={t('immich.filter.album')}>
        <select
          className={fieldClass}
          value={albumValue}
          onChange={(e) => {
            const value = e.target.value;
            update({
              albumId: value && value !== NOT_IN_ALBUM ? value : null,
              notInAlbum: value === NOT_IN_ALBUM,
            });
          }}
        >
          <option value="">{t('immich.filter.any')}</option>
          <option value={NOT_IN_ALBUM}>{t('immich.section.unassigned')}</option>
          {albums.map((a) => (
            <option key={a.id} value={a.id}>
              {a.albumName}
            </option>
          ))}
        </select>
      </Row>

      <div className="grid grid-cols-2 gap-2">
        <Row label={t('immich.filter.from')}>
          <input
            type="date"
            className={fieldClass}
            value={filter.takenFrom ?? ''}
            onChange={(e) => update({ takenFrom: e.target.value || null })}
          />
        </Row>
        <Row label={t('immich.filter.until')}>
          <input
            type="date"
            className={fieldClass}
            value={filter.takenUntil ?? ''}
            onChange={(e) => update({ takenUntil: e.target.value || null })}
          />
        </Row>
      </div>

      <Row label={t('immich.filter.country')}>
        <Choice
          value={filter.country}
          options={asOptions(countries)}
          anyLabel={t('immich.filter.any')}
          onChange={(country) => update({ country, city: null })}
        />
      </Row>
      <Row label={t('immich.filter.city')}>
        <Choice
          value={filter.city}
          options={asOptions(cities)}
          anyLabel={t('immich.filter.any')}
          onChange={(city) => update({ city })}
        />
      </Row>

      <div className="grid grid-cols-2 gap-2">
        <Row label={t('immich.filter.make')}>
          <Choice
            value={filter.make}
            options={asOptions(makes)}
            anyLabel={t('immich.filter.any')}
            onChange={(make) => update({ make, model: null })}
          />
        </Row>
        <Row label={t('immich.filter.model')}>
          <Choice
            value={filter.model}
            options={asOptions(models)}
            anyLabel={t('immich.filter.any')}
            onChange={(model) => update({ model })}
          />
        </Row>
      </div>

      <Row label={t('immich.filter.person')}>
        <Choice
          value={filter.personIds?.[0]}
          options={people.map((p) => ({ value: p.id, label: p.name }))}
          anyLabel={t('immich.filter.any')}
          onChange={(id) => update({ personIds: id ? [id] : [] })}
        />
      </Row>

      <label className="flex items-center gap-2 cursor-pointer select-none">
        <input
          type="checkbox"
          className="accent-accent"
          checked={!!filter.favoritesOnly}
          onChange={(e) => update({ favoritesOnly: e.target.checked })}
        />
        <Text variant={TextVariants.small}>{t('immich.filter.favoritesOnly')}</Text>
      </label>

      <div className="flex gap-2 pt-1">
        <button
          type="submit"
          className="flex-1 h-8 flex items-center justify-center gap-1.5 rounded-md bg-accent text-button-text text-sm font-semibold"
        >
          <Search size={14} />
          {t('immich.filter.apply')}
        </button>
        <button
          type="button"
          className="h-8 px-2 flex items-center justify-center rounded-md bg-surface text-text-secondary hover:text-text-primary"
          data-tooltip={t('immich.filter.reset')}
          onClick={() => setFilter({})}
        >
          <X size={14} />
        </button>
      </div>
    </form>
  );
}

export function describeFilter(filter: ImmichFilter, albums: ImmichAlbum[], fallback: string, unassigned: string) {
  const parts: string[] = [];
  if (filter.albumId) parts.push(albums.find((a) => a.id === filter.albumId)?.albumName ?? fallback);
  if (filter.notInAlbum) parts.push(unassigned);
  const day = (d: string) => new Date(`${d}T00:00:00`).toLocaleDateString();
  if (filter.takenFrom || filter.takenUntil) {
    parts.push(`${filter.takenFrom ? day(filter.takenFrom) : '…'}–${filter.takenUntil ? day(filter.takenUntil) : '…'}`);
  }
  for (const value of [filter.city || filter.country, filter.model || filter.make]) {
    if (value) parts.push(value);
  }
  if (filter.favoritesOnly) parts.push('★');
  return parts.length ? parts.join(' · ') : fallback;
}
