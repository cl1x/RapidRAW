import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { toast } from 'react-toastify';
import { useTranslation } from 'react-i18next';
import { useLibraryStore } from '../../store/useLibraryStore';
import { useImmichStore } from './useImmichStore';

interface DownloadEvent {
  path: string;
  state: 'started' | 'done' | 'error';
  error?: string;
}

interface UploadEvent {
  fileName: string;
  state: 'started' | 'done' | 'error';
  error?: string;
}

type TransferEvent =
  | { state: 'progress'; current: number; total: number }
  | { state: 'done'; added: number; failed: number; error?: string };

const TRANSFER_TOAST = 'immich-transfer';

/**
 * Reports downloads, uploads and album changes of Immich images, wherever the
 * user is, and reloads the library when Immich changed underneath it.
 */
export function useImmichEvents(refreshLibrary: () => void) {
  const { t } = useTranslation();
  const refreshRef = useRef(refreshLibrary);
  refreshRef.current = refreshLibrary;

  useEffect(() => {
    const unlisteners = [
      listen<DownloadEvent>('immich-download', ({ payload }) => {
        if (payload.state === 'done') {
          useLibraryStore.getState().setLibrary((state) => ({
            imageList: state.imageList.map((image) =>
              image.path === payload.path ? { ...image, is_cloud_placeholder: false } : image,
            ),
          }));
        } else if (payload.state === 'error') {
          toast.error(t('immich.toasts.downloadFailed', { error: payload.error }));
        }
      }),
      listen<UploadEvent>('immich-upload', ({ payload }) => {
        if (payload.state === 'done') {
          toast.success(t('immich.toasts.uploaded', { fileName: payload.fileName }));
        } else if (payload.state === 'error') {
          toast.error(t('immich.toasts.uploadFailed', { fileName: payload.fileName, error: payload.error }));
        }
      }),
      listen<TransferEvent>('immich-transfer', ({ payload }) => {
        if (payload.state === 'progress') {
          const text = t('immich.toasts.transferring', { current: payload.current + 1, total: payload.total });
          if (toast.isActive(TRANSFER_TOAST)) toast.update(TRANSFER_TOAST, { render: text });
          else toast.loading(text, { toastId: TRANSFER_TOAST });
          return;
        }
        toast.dismiss(TRANSFER_TOAST);
        if (payload.failed > 0) {
          toast.error(t('immich.toasts.transferFailed', { count: payload.failed, error: payload.error }));
        }
        if (payload.added > 0) {
          toast.success(t('immich.toasts.transferred', { count: payload.added }));
        }
      }),
      listen('immich-library-changed', () => {
        useImmichStore.getState().loadAlbums();
        refreshRef.current();
      }),
    ];

    return () => {
      unlisteners.forEach((unlisten) => unlisten.then((fn) => fn()));
    };
  }, [t]);
}
