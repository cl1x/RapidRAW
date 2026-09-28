import { create } from 'zustand';
import { getImmichConfig, ImmichAlbum, listImmichAlbums } from './immichApi';

interface ImmichState {
  isConfigured: boolean;
  albums: ImmichAlbum[];
  isLoading: boolean;
  error: string | null;
  /** Re-reads the settings, and the albums if Immich is set up. */
  refresh(): Promise<void>;
  loadAlbums(): Promise<void>;
}

export const useImmichStore = create<ImmichState>((set, get) => ({
  isConfigured: false,
  albums: [],
  isLoading: false,
  error: null,

  refresh: async () => {
    try {
      const config = await getImmichConfig();
      const isConfigured = !!config.serverUrl.trim() && !!config.apiKey.trim();
      set({ isConfigured, ...(isConfigured ? {} : { albums: [], error: null }) });
      if (isConfigured) await get().loadAlbums();
    } catch (err) {
      console.error('Failed to read Immich settings:', err);
    }
  },

  loadAlbums: async () => {
    set({ isLoading: true, error: null });
    try {
      set({ albums: await listImmichAlbums() });
    } catch (err) {
      set({ error: String(err) });
    } finally {
      set({ isLoading: false });
    }
  },
}));
