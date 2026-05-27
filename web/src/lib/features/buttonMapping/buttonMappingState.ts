import { steamBindingTargetPart } from './buttonMapping';

export type RawBindingTargetGroup = {
  label: string;
  options: Array<{ label: string; raw: string }>;
};

export type PreparedSteamBindingTargetGroup = {
  label: string;
  options: Array<{ label: string; raw: string; targetKey: string; searchText: string }>;
};

export function prepareBindingTargetGroups(groups: RawBindingTargetGroup[]): PreparedSteamBindingTargetGroup[] {
  return groups.map((group) => ({
    label: group.label,
    options: group.options.map((option) => ({
      ...option,
      targetKey: steamBindingTargetPart(option.raw),
      searchText: `${group.label} ${option.label} ${option.raw}`.toLowerCase()
    }))
  }));
}

export function bindingTargetGroupsForProvider(
  groups: PreparedSteamBindingTargetGroup[],
  provider: 'steam' | 'bridge'
): PreparedSteamBindingTargetGroup[] {
  if (provider === 'steam') return groups;
  return groups
    .map((group) => ({
      ...group,
      options: group.options.filter((option) => {
        const raw = option.raw.trim().toLowerCase();
        return raw.startsWith('xinput_button ') && !raw.includes('touchpad');
      })
    }))
    .filter((group) => group.options.length > 0);
}
