import { create } from 'zustand';
import {
  getImmichConfig,
  ImmichAlbum,
  ImmichFilter,
  ImmichTimelineMonth,
  listImmichAlbums,
  listImmichTimeline,
} from './immichApi';

interface ImmichState {
  isConfigured: boolean;
  albums: ImmichAlbum[];
  isLoading: boolean;
  error: string | null;
  /** Months with photos, newest first; loaded when the timeline is opened. */
  timeline: ImmichTimelineMonth[];
  loadTimeline(): Promise<void>;
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
  timeline: [],
  loadTimeline: async () => {
    try {
      set({ timeline: await listImmichTimeline() });
    } catch (err) {
      console.error('Failed to load the Immich timeline:', err);
    }
  },
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
