import type { SteamLibraryBrowseEntry, SteamLibraryEntry } from '../../types';

export type SteamExecutableSelection = {
  name: string;
  relativePath: string;
};

export type SteamBrowseCrumb = {
  label: string;
  path: string;
};

export function filterSteamLibraryEntries(
  entries: SteamLibraryEntry[],
  query: string
): SteamLibraryEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return entries;
  return entries.filter((entry) => {
    return (
      entry.name.toLowerCase().includes(q) ||
      entry.appId.includes(q) ||
      entry.installDir.toLowerCase().includes(q)
    );
  });
}

export function countAvailableSteamEntries(entries: SteamLibraryEntry[]): number {
  return entries.filter((entry) => !entry.alreadyInCatalog).length;
}

export function formatPlaytime(minutes: number | null | undefined): string {
  if (!minutes || minutes <= 0) return '';
  if (minutes < 60) return `${minutes}m played`;
  const hours = minutes / 60;
  return `${hours.toFixed(hours < 10 ? 1 : 0)}h played`;
}

export function steamEntryArt(entry: SteamLibraryEntry): string | null {
  return (
    entry.artwork?.capsuleUrl ??
    entry.artwork?.bannerUrl ??
    entry.artwork?.heroUrl ??
    entry.artwork?.iconUrl ??
    null
  );
}

export function initialExecutableSelection(entry: SteamLibraryEntry): SteamExecutableSelection[] {
  return entry.processCandidates.map((name) => ({ name, relativePath: name }));
}

export function joinBrowsePath(parent: string, child: string): string {
  if (!parent) return child;
  if (!child) return parent;
  return `${parent}/${child}`;
}

export function isExecutableSelected(
  selected: SteamExecutableSelection[],
  entry: SteamLibraryBrowseEntry
): boolean {
  if (entry.kind !== 'exe') return false;
  return selected.some((item) => item.name.toLowerCase() === entry.name.toLowerCase());
}

export function formatBrowseSize(bytes: number | null | undefined): string {
  if (!bytes || bytes <= 0) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function buildSteamBrowseBreadcrumbs(
  entry: SteamLibraryEntry | null,
  browsePath: string
): SteamBrowseCrumb[] {
  if (!entry) return [];
  const crumbs: SteamBrowseCrumb[] = [{ label: entry.installDir || 'root', path: '' }];
  if (!browsePath) return crumbs;

  const parts = browsePath.split('/').filter((part) => part.length > 0);
  let acc = '';
  for (const part of parts) {
    acc = acc ? `${acc}/${part}` : part;
    crumbs.push({ label: part, path: acc });
  }
  return crumbs;
}
