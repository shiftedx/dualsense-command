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
    (id === 'advancedButtonMapping' && !readiness.buttonMappingReady);

  const itemTooltip = (id: AppView): string => {
    if (id === 'tuning' && !readiness.tuningReady) return 'Select a controller before tuning haptics.';
    if (id === 'advancedButtonMapping' && !readiness.buttonMappingReady) {
      return 'Select a game or local app scope before editing mappings.';
    }
    return viewTooltips[id];
  };
</script>

<nav class="sidebar" aria-label="Main">
  <div class="sidebar-brand">DSCC</div>
  {#each main as item (item.id)}
    <button
      class="sidebar-item"
      class:active={view === item.id}
      type="button"
      title={itemTooltip(item.id)}
      disabled={itemDisabled(item.id)}
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
        type="button"
        title={itemTooltip(item.id)}
        disabled={itemDisabled(item.id)}
        aria-current={view === item.id ? 'page' : undefined}
        onclick={() => onNavigate(item.id)}
      >{item.label}</button>
    {/each}
  {/if}
  <div class="sidebar-spacer"></div>
  {@render footer?.()}
</nav>
