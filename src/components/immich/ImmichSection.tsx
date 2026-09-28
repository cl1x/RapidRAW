import { ReactNode, useEffect, useMemo, useState } from 'react';
import { AnimatePresence, motion } from 'framer-motion';
import clsx from 'clsx';
import { useDroppable } from '@dnd-kit/core';
import {
  Album as AlbumIcon,
  CalendarDays,
  ChevronDown,
  ChevronRight,
  History,
  Inbox,
  MoveRight,
  RefreshCw,
  SlidersHorizontal,
  Users,
} from 'lucide-react';
import { LucideIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import Text from '../ui/Text';
import { TextColors, TextVariants, TextWeights } from '../../types/typography';
import { useUIStore } from '../../store/useUIStore';
import {
  filterToAlbumId,
  IMMICH_UNASSIGNED_ID,
  ImmichFilter,
  isImmichFilterId,
  monthFilter,
  toImmichAlbumId,
} from './immichApi';
import ImmichFilterForm, { describeFilter } from './ImmichFilterForm';
import { useImmichStore } from './useImmichStore';

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
  const [isFilterOpen, setFilterOpen] = useState(false);

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

function ImmichTimeline({
  selectedAlbumId,
  onSelectAlbum,
}: {
  selectedAlbumId: string | null;
  onSelectAlbum(albumId: string, albumName: string, images: string[]): void;
}) {
  const { t, i18n } = useTranslation();
  const { timeline, loadTimeline } = useImmichStore(
    useShallow((state) => ({ timeline: state.timeline, loadTimeline: state.loadTimeline })),
  );
  const [isOpen, setOpen] = useState(false);
  const [openYear, setOpenYear] = useState<string | null>(null);

  const years = useMemo(() => {
    const byYear = new Map<string, { month: string; count: number }[]>();
    for (const bucket of timeline) {
      const year = bucket.timeBucket.slice(0, 4);
      byYear.set(year, [...(byYear.get(year) ?? []), { month: bucket.timeBucket, count: bucket.count }]);
    }
    return [...byYear.entries()];
  }, [timeline]);

  const monthName = (bucket: string) =>
    new Date(`${bucket.slice(0, 7)}-01T00:00:00`).toLocaleDateString(i18n.language, { month: 'long', year: 'numeric' });

  const toggle = () => {
    if (!isOpen) loadTimeline();
    setOpen(!isOpen);
  };

  return (
    <>
      <ImmichRow
        icon={History}
        label={t('immich.section.timeline')}
        isSelected={false}
        onSelect={toggle}
        trailing={isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      />
      {isOpen &&
        years.map(([year, months]) => (
          <div key={year} className="pl-4">
            <ImmichRow
              icon={CalendarDays}
              label={year}
              count={months.reduce((sum, m) => sum + m.count, 0)}
              showCount
              isSelected={false}
              onSelect={() => setOpenYear(openYear === year ? null : year)}
            />
            {openYear === year &&
              months.map(({ month, count }) => {
                const id = filterToAlbumId(monthFilter(month));
                return (
                  <div key={month} className="pl-4">
                    <ImmichRow
                      icon={CalendarDays}
                      label={monthName(month)}
                      count={count}
                      showCount
                      isSelected={selectedAlbumId === id}
                      onSelect={() => onSelectAlbum(id, monthName(month), [])}
                    />
                  </div>
                );
              })}
          </div>
        ))}
    </>
  );
}

// Drops are handled by the library's drop handler, which passes them to
// `add_to_album` as for local albums.
function ImmichRow({
  icon,
  label,
  count,
  showCount = false,
  dropId,
  isSelected,
  onSelect,
  tooltip,
  trailing,
}: {
  icon: LucideIcon;
  label: string;
  count?: number;
  showCount?: boolean;
  dropId?: string;
  isSelected: boolean;
  onSelect(): void;
  tooltip?: string;
  trailing?: ReactNode;
}) {
  const isLayoutDragging = useUIStore((state) => !!state.activeLayoutDragItem);
  const { setNodeRef, isOver, active } = useDroppable({
    id: `album-immich-${dropId ?? label}`,
    data: { type: 'album', id: dropId },
    disabled: !dropId || isLayoutDragging,
  });
  const isDropTarget = !!dropId && isOver && active?.data?.current?.type === 'library-image';
  const Icon = isDropTarget ? MoveRight : icon;

  return (
    <Text as="div" color={TextColors.primary} weight={TextWeights.medium}>
      <div
        ref={setNodeRef}
        className={clsx('flex items-center gap-2 p-1.5 rounded-md transition-colors cursor-pointer', {
          'bg-surface': isSelected && !isDropTarget,
          'hover:bg-card-active': !isSelected && !isDropTarget,
          'bg-accent/20': isDropTarget,
        })}
        onClick={onSelect}
        data-tooltip={tooltip}
      >
        <div className="w-5 h-5 flex items-center justify-center p-0.5 rounded-sm text-text-secondary shrink-0">
          <Icon size={16} />
        </div>
        <span className="min-w-0 flex-1 select-none">
          <span className="block truncate">{label}</span>
        </span>
        {count !== undefined && (
          <Text
            as="span"
            variant={TextVariants.small}
            color={TextColors.secondary}
            className={clsx(
              'ml-auto min-w-8 shrink-0 text-right tabular-nums transition-opacity ease-in-out duration-300',
              showCount ? 'opacity-100' : 'opacity-0',
            )}
          >
            {count}
          </Text>
        )}
        <div className="w-5 h-5 shrink-0 flex items-center justify-center text-text-secondary" aria-hidden="true">
          {trailing}
        </div>
      </div>
    </Text>
  );
}
