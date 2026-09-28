import { invoke } from '@tauri-apps/api/core';
import { ImageFile } from '../ui/AppProperties';

/**
 * Immich albums share the library's album flow. Their ids carry this prefix so
 * they never collide with local albums and can be told apart wherever the
 * active album id is handled.
 */
export const IMMICH_ALBUM_PREFIX = 'immich:';

export const isImmichAlbumId = (albumId: string | null | undefined): albumId is string =>
  !!albumId && albumId.startsWith(IMMICH_ALBUM_PREFIX);

export const toImmichAlbumId = (immichId: string) => `${IMMICH_ALBUM_PREFIX}${immichId}`;

export const ImmichInvokes = {
  GetConfig: 'immich_get_config',
  SaveConfig: 'immich_save_config',
  TestConnection: 'immich_test_connection',
  ListAlbums: 'immich_list_albums',
  GetAlbumImages: 'immich_get_album_images',
} as const;

export interface ImmichConfig {
  serverUrl: string;
  apiKey: string;
  preferRaw: boolean;
  uploadExports: boolean;
  replacePreviousExport: boolean;
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

export interface ImmichConnectionInfo {
  version: string;
  userName: string;
  userEmail: string;
}

export const getImmichConfig = () => invoke<ImmichConfig>(ImmichInvokes.GetConfig);

export const saveImmichConfig = (config: ImmichConfig) => invoke<void>(ImmichInvokes.SaveConfig, { config });

export const testImmichConnection = (serverUrl: string, apiKey: string) =>
  invoke<ImmichConnectionInfo>(ImmichInvokes.TestConnection, { serverUrl, apiKey });

export const listImmichAlbums = () => invoke<ImmichAlbum[]>(ImmichInvokes.ListAlbums);

/** Takes the prefixed album id used in the library. */
export const getImmichAlbumImages = (albumId: string) =>
  invoke<ImageFile[]>(ImmichInvokes.GetAlbumImages, {
    albumId: albumId.slice(IMMICH_ALBUM_PREFIX.length),
  });
