import { invoke } from '@tauri-apps/api/core';
import { ImageFile } from '../ui/AppProperties';

/**
 * Immich listings share the library's album flow. Their ids carry this prefix
 * so they never collide with local albums and can be told apart wherever the
 * active album id is handled. After the prefix comes an Immich album id, one
 * of the pseudo albums below, or `?` and a filter as JSON.
 */
export const IMMICH_ALBUM_PREFIX = 'immich:';
export const IMMICH_ALL_ID = `${IMMICH_ALBUM_PREFIX}all`;
export const IMMICH_UNASSIGNED_ID = `${IMMICH_ALBUM_PREFIX}unassigned`;
const FILTER_MARKER = '?';

export const isImmichAlbumId = (albumId: string | null | undefined): albumId is string =>
  !!albumId && albumId.startsWith(IMMICH_ALBUM_PREFIX);

export const toImmichAlbumId = (immichId: string) => `${IMMICH_ALBUM_PREFIX}${immichId}`;

export const isImmichFilterId = (albumId: string | null | undefined) =>
  isImmichAlbumId(albumId) && albumId.startsWith(`${IMMICH_ALBUM_PREFIX}${FILTER_MARKER}`);

export const ImmichInvokes = {
  GetConfig: 'immich_get_config',
  SaveConfig: 'immich_save_config',
  TestConnection: 'immich_test_connection',
  ListAlbums: 'immich_list_albums',
  GetImages: 'immich_get_images',
  Suggestions: 'immich_suggestions',
  ListPeople: 'immich_list_people',
  Timeline: 'immich_timeline',
} as const;

export interface ImmichConfig {
  serverUrl: string;
  apiKey: string;
  preferRaw: boolean;
  uploadExports: boolean;
  replacePreviousExport: boolean;
  rawLeavesAlbum: boolean;
  cacheDir: string | null;
  cacheLimitGb: number;
}

export interface ImmichAlbum {
  id: string;
  albumName: string;
  assetCount: number;
  albumThumbnailAssetId: string | null;
  startDate: string | null;
  endDate: string | null;
  shared: boolean;
}

export interface ImmichTimelineMonth {
  /** First day of the month, `YYYY-MM-DD`. */
  timeBucket: string;
  count: number;
}

export interface ImmichPerson {
  id: string;
  name: string;
}

export interface ImmichConnectionInfo {
  version: string;
  userName: string;
  userEmail: string;
}

/** Mirrors `resolve::Filter` in the backend. Empty fields do not filter. */
export interface ImmichFilter {
  albumId?: string | null;
  notInAlbum?: boolean;
  /** `YYYY-MM-DD`, both days included. */
  takenFrom?: string | null;
  takenUntil?: string | null;
  country?: string | null;
  city?: string | null;
  make?: string | null;
  model?: string | null;
  personIds?: string[];
  favoritesOnly?: boolean;
}

export type SuggestionKind = 'country' | 'city' | 'camera-make' | 'camera-model';

export const filterToAlbumId = (filter: ImmichFilter) =>
  `${IMMICH_ALBUM_PREFIX}${FILTER_MARKER}${JSON.stringify(filter)}`;

export const albumIdToFilter = (albumId: string): ImmichFilter => {
  const rest = albumId.slice(IMMICH_ALBUM_PREFIX.length);
  if (rest === 'all') return {};
  if (rest === 'unassigned') return { notInAlbum: true };
  if (rest.startsWith(FILTER_MARKER)) {
    try {
      return JSON.parse(rest.slice(FILTER_MARKER.length));
    } catch {
      return {};
    }
  }
  return { albumId: rest };
};

export const getImmichConfig = () => invoke<ImmichConfig>(ImmichInvokes.GetConfig);

export const saveImmichConfig = (config: ImmichConfig) => invoke<void>(ImmichInvokes.SaveConfig, { config });

export const testImmichConnection = (serverUrl: string, apiKey: string) =>
  invoke<ImmichConnectionInfo>(ImmichInvokes.TestConnection, { serverUrl, apiKey });

export const listImmichAlbums = () => invoke<ImmichAlbum[]>(ImmichInvokes.ListAlbums);

export const getImmichSuggestions = (kind: SuggestionKind, narrow: { country?: string; make?: string } = {}) =>
  invoke<string[]>(ImmichInvokes.Suggestions, {
    kind,
    country: narrow.country || null,
    make: narrow.make || null,
  });

export const listImmichPeople = () => invoke<ImmichPerson[]>(ImmichInvokes.ListPeople);

export const listImmichTimeline = () => invoke<ImmichTimelineMonth[]>(ImmichInvokes.Timeline);

/** The filter for one month of the timeline. */
export const monthFilter = (timeBucket: string): ImmichFilter => {
  const [year, month] = timeBucket.split('-').map(Number);
  const lastDay = new Date(year, month, 0).getDate();
  const mm = String(month).padStart(2, '0');
  return { takenFrom: `${year}-${mm}-01`, takenUntil: `${year}-${mm}-${String(lastDay).padStart(2, '0')}` };
};

/** Takes an id from the library: prefixed album id, pseudo album or filter. */
export const getImmichAlbumImages = (albumId: string) =>
  invoke<ImageFile[]>(ImmichInvokes.GetImages, { filter: albumIdToFilter(albumId) });
