import { create } from 'zustand';
import { getImmichConfig, ImmichAlbum, ImmichFilter, listImmichAlbums } from './immichApi';

interface ImmichState {
  isConfigured: boolean;
  albums: ImmichAlbum[];
  isLoading: boolean;
  error: string | null;
  /** The filter last used in the sidebar, kept while the app runs. */
  filter: ImmichFilter;
  setFilter(filter: ImmichFilter): void;
  /** Re-reads the settings, and the albums if Immich is set up. */
  refresh(): Promise<void>;
  loadAlbums(): Promise<void>;
}

export const useImmichStore = create<ImmichState>((set, get) => ({
  isConfigured: false,
  albums: [],
  isLoading: false,
  error: null,
  filter: {},
  setFilter: (filter) => set({ filter }),

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
