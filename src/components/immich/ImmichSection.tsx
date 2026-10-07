import { ReactNode, useEffect, useMemo, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import clsx from 'clsx';
import { Album as AlbumIcon, Inbox, RefreshCw, SlidersHorizontal, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import Text from '../ui/Text';
import { TextVariants, TextWeights } from '../../types/typography';
import { filterToAlbumId, IMMICH_UNASSIGNED_ID, ImmichFilter, isImmichFilterId, toImmichAlbumId } from './immichApi';
import ImmichFilterForm, { describeFilter } from './ImmichFilterForm';
import ImmichRow from './ImmichRow';
import ImmichTimeline from './ImmichTimeline';
import { useImmichStore } from './useImmichStore';
import { useSettingsStore } from '../../store/useSettingsStore';

interface ImmichSectionProps {
  header: ReactNode;
  isOpen: boolean;
  onSelectAlbum(albumId: string, albumName: string, images: string[]): void;
  searchQuery: string;
  selectedAlbumId: string | null;
  showImageCounts: boolean;
}

export default function ImmichSection({
  header,
  isOpen,
  onSelectAlbum,
  searchQuery,
  selectedAlbumId,
  showImageCounts,
}: ImmichSectionProps) {
  const { t } = useTranslation();
  const { isConfigured, albums, isLoading, error, filter, setFilter, refresh, loadAlbums } = useImmichStore(
    useShallow((state) => ({
      isConfigured: state.isConfigured,
      albums: state.albums,
      isLoading: state.isLoading,
      error: state.error,
      filter: state.filter,
      setFilter: state.setFilter,
      refresh: state.refresh,
      loadAlbums: state.loadAlbums,
    })),
  );
  const serverUrl = useSettingsStore((state) => state.appSettings?.immich?.serverUrl);
  const albumSort = useSettingsStore((state) => state.appSettings?.immich?.albumSort);
  const [isFilterOpen, setFilterOpen] = useState(false);

  useEffect(() => {
    refresh();
  }, [refresh, serverUrl, albumSort]);

  const query = searchQuery.trim().toLowerCase();
  const visibleAlbums = useMemo(
    () => (query ? albums.filter((a) => a.albumName.toLowerCase().includes(query)) : albums),
    [albums, query],
  );

  if (!isConfigured || (query && visibleAlbums.length === 0)) {
    return null;
  }

  const isFilterSelected = isImmichFilterId(selectedAlbumId);

  const applyFilter = (next: ImmichFilter) => {
    setFilter(next);
    setFilterOpen(false);
    const title = describeFilter(next, albums, t('immich.section.all'), t('immich.section.unassigned'));
    onSelectAlbum(filterToAlbumId(next), title, []);
  };

  return (
    <>
      <div>{header}</div>
      <AnimatePresence initial={false}>
        {isOpen && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: 'easeInOut' }}
            className="overflow-hidden"
          >
            <div className="pt-1 pb-2">
              {!query && (
                <>
                  <ImmichTimeline selectedAlbumId={selectedAlbumId} onSelectAlbum={onSelectAlbum} />
                  <ImmichRow
                    icon={Inbox}
                    label={t('immich.section.unassigned')}
                    dropId={IMMICH_UNASSIGNED_ID}
                    isSelected={selectedAlbumId === IMMICH_UNASSIGNED_ID}
                    onSelect={() => onSelectAlbum(IMMICH_UNASSIGNED_ID, t('immich.section.unassigned'), [])}
                    tooltip={t('immich.section.unassignedHint')}
                  />
                  <ImmichRow
                    icon={SlidersHorizontal}
                    label={t('immich.section.filter')}
                    isSelected={isFilterSelected}
                    onSelect={() => setFilterOpen((open) => !open)}
                  />
                  <AnimatePresence initial={false}>
                    {isFilterOpen && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        className="overflow-hidden"
                      >
                        <ImmichFilterForm albums={albums} initial={filter} onApply={applyFilter} />
                      </motion.div>
                    )}
                  </AnimatePresence>
                  <div className="my-1 mx-2 h-px bg-surface" />
                </>
              )}

              {visibleAlbums.map((album) => (
                <ImmichRow
                  key={album.id}
                  icon={album.shared ? Users : AlbumIcon}
                  label={album.albumName}
                  count={album.assetCount}
                  showCount={showImageCounts}
                  dropId={toImmichAlbumId(album.id)}
                  isSelected={toImmichAlbumId(album.id) === selectedAlbumId}
                  onSelect={() => onSelectAlbum(toImmichAlbumId(album.id), album.albumName, [])}
                />
              ))}

              {error && (
                <Text variant={TextVariants.small} className="p-2 text-center" data-tooltip={error}>
                  {t('immich.section.error')}
                </Text>
              )}
              {!error && !isLoading && albums.length === 0 && (
                <Text variant={TextVariants.small} className="p-2 text-center">
                  {t('immich.section.empty')}
                </Text>
              )}

              {!query && (
                <Text
                  as="div"
                  weight={TextWeights.medium}
                  className="flex items-center gap-2 p-2 mt-1 rounded-md transition-opacity opacity-70 hover:opacity-100 hover:bg-card-active cursor-pointer hover:text-text-primary"
                  onClick={() => !isLoading && loadAlbums()}
                >
                  <div className="relative w-4 h-4 ml-1 shrink-0 flex items-center justify-center">
                    <RefreshCw size={14} className={clsx(isLoading && 'animate-spin')} />
                  </div>
                  <span className="select-none">
                    {isLoading ? t('immich.section.loading') : t('immich.section.refresh')}
                  </span>
                </Text>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
