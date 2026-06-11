<script lang="ts">
  import type { Snippet } from 'svelte';
  import { appViews, viewTooltips, type AppView, type ViewReadiness } from '../app/navigation';

  let {
    view,
    readiness,
    onNavigate,
    footer
  }: {
    view: AppView;
    readiness: ViewReadiness;
    onNavigate: (view: AppView) => void;
    footer?: Snippet;
  } = $props();

  // svelte-ignore state_referenced_locally -- the initial value is intentional;
  // the $effect below keeps the group open whenever the view enters it.
  let advancedOpen = $state(view.startsWith('advanced'));
  $effect(() => {
    if (view.startsWith('advanced')) advancedOpen = true;
  });

  const main = appViews.filter((item) => item.group === 'main');
  const advanced = appViews.filter((item) => item.group === 'advanced');

  const itemDisabled = (id: AppView): boolean =>
    (id === 'tuning' && !readiness.tuningReady) ||
    (id === 'advancedButtonMapping' && !readiness.buttonMappingReady) ||
    (id === 'advancedEdgeSlots' && !readiness.edgeSlotsReady);

  const itemTooltip = (id: AppView): string => {
    if (id === 'tuning' && !readiness.tuningReady) return 'Select a controller before tuning haptics.';
    if (id === 'advancedButtonMapping' && !readiness.buttonMappingReady) {
      return 'Pick a supported game in Tuning to map its buttons.';
    }
    if (id === 'advancedEdgeSlots' && !readiness.edgeSlotsReady) {
      return 'Onboard slots are available when the Target Controller is a DualSense Edge.';
    }
    return viewTooltips[id];
  };
</script>

<nav class="sidebar" aria-label="Main">
  <div class="sidebar-brand" title="DualSense Command Center">
    <span class="dm-controller-glyph sidebar-brand-glyph" aria-hidden="true"></span>
    <span class="visually-hidden">DualSense Command Center</span>
  </div>
  {#each main as item (item.id)}
    <button
      class="sidebar-item"
      class:active={view === item.id}
      class:disabled={itemDisabled(item.id)}
      type="button"
      title={itemTooltip(item.id)}
      aria-disabled={itemDisabled(item.id) ? 'true' : undefined}
      aria-current={view === item.id ? 'page' : undefined}
      onclick={() => onNavigate(item.id)}
    >{item.label}</button>
  {/each}
  <button
    class="sidebar-group"
    type="button"
    aria-expanded={advancedOpen}
    onclick={() => (advancedOpen = !advancedOpen)}
  >Advanced {advancedOpen ? '▾' : '▸'}</button>
  {#if advancedOpen}
    {#each advanced as item (item.id)}
      <button
        class="sidebar-item sidebar-sub"
        class:active={view === item.id}
        class:disabled={itemDisabled(item.id)}
        type="button"
        title={itemTooltip(item.id)}
        aria-disabled={itemDisabled(item.id) ? 'true' : undefined}
        aria-current={view === item.id ? 'page' : undefined}
        onclick={() => onNavigate(item.id)}
      >{item.label}</button>
    {/each}
  {/if}
  <div class="sidebar-spacer"></div>
  {@render footer?.()}
</nav>
