import { ReactNode, useEffect, useMemo } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import clsx from 'clsx';
import { Album as AlbumIcon, RefreshCw, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import Text from '../ui/Text';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';
import { ImmichAlbum, toImmichAlbumId } from './immichApi';
import { useImmichStore } from './useImmichStore';

interface ImmichSectionProps {
  /** The section header, so it looks like the other sections of the tree. */
  header: ReactNode;
  isOpen: boolean;
  onSelectAlbum(albumId: string, albumName: string, images: string[]): void;
  searchQuery: string;
  selectedAlbumId: string | null;
  showImageCounts: boolean;
}

/**
 * The albums of the connected Immich server, as a section of the folder tree.
 * Renders nothing until Immich is set up in the settings.
 */
export default function ImmichSection({
  header,
  isOpen,
  onSelectAlbum,
  searchQuery,
  selectedAlbumId,
  showImageCounts,
}: ImmichSectionProps) {
  const { t } = useTranslation();
  const { isConfigured, albums, isLoading, error, refresh, loadAlbums } = useImmichStore(
    useShallow((state) => ({
      isConfigured: state.isConfigured,
      albums: state.albums,
      isLoading: state.isLoading,
      error: state.error,
      refresh: state.refresh,
      loadAlbums: state.loadAlbums,
    })),
  );

  useEffect(() => {
    refresh();
  }, [refresh]);

  const query = searchQuery.trim().toLowerCase();
  const visibleAlbums = useMemo(
    () => (query ? albums.filter((a) => a.albumName.toLowerCase().includes(query)) : albums),
    [albums, query],
  );

  if (!isConfigured || (query && visibleAlbums.length === 0)) {
    return null;
  }

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
              {visibleAlbums.map((album) => (
                <ImmichAlbumRow
                  key={album.id}
                  album={album}
                  isSelected={toImmichAlbumId(album.id) === selectedAlbumId}
                  onSelect={() => onSelectAlbum(toImmichAlbumId(album.id), album.albumName, [])}
                  showImageCount={showImageCounts}
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

function ImmichAlbumRow({
  album,
  isSelected,
  onSelect,
  showImageCount,
}: {
  album: ImmichAlbum;
  isSelected: boolean;
  onSelect(): void;
  showImageCount: boolean;
}) {
  const ItemIcon = album.shared ? Users : AlbumIcon;

  return (
    <Text as="div" color={TextColors.primary} weight={TextWeights.medium}>
      <div
        className={clsx('flex items-center gap-2 p-1.5 rounded-md transition-colors cursor-pointer', {
          'bg-surface': isSelected,
          'hover:bg-card-active': !isSelected,
        })}
        onClick={onSelect}
      >
        <div className="w-5 h-5 flex items-center justify-center p-0.5 rounded-sm text-text-secondary shrink-0">
          <ItemIcon size={16} />
        </div>
        <span className="min-w-0 flex-1 select-none">
          <span className="block truncate">{album.albumName}</span>
        </span>
        <Text
          as="span"
          variant={TextVariants.small}
          color={TextColors.secondary}
          className={clsx(
            'ml-auto min-w-8 shrink-0 text-right tabular-nums transition-opacity ease-in-out duration-300',
            showImageCount ? 'opacity-100' : 'opacity-0',
          )}
        >
          {album.assetCount}
        </Text>
        <div className="w-5 h-5 shrink-0" aria-hidden="true" />
      </div>
    </Text>
  );
}
