<script lang="ts">
  import Tooltip from './Tooltip.svelte';
  import type { AppView, AppViewDefinition } from '../app/navigation';

  export let views: AppViewDefinition[] = [];
  export let activeView: AppView = 'games';
  export let tooltips: Record<AppView, string>;
  export let tuningReady = false;
  export let buttonMappingReady = false;
  export let onNavigate: (view: AppView) => void = () => {};

  $: viewModels = views.map((view) => {
    const disabled = (view.id === 'haptics' && !tuningReady) || (view.id === 'buttonMapping' && !buttonMappingReady);
    const tooltip =
      view.id === 'buttonMapping' && !buttonMappingReady
        ? 'Select a game or local app scope before editing mappings.'
        : view.id === 'haptics' && !tuningReady
          ? 'Select a controller before tuning haptics.'
          : tooltips[view.id];
    return { ...view, disabled, tooltip };
  });
</script>

<nav class="dm-view-nav" aria-label="Command center views">
  {#each viewModels as view}
    <Tooltip text={view.tooltip} side="bottom" align="center">
      <button
        class:active={activeView === view.id}
        disabled={view.disabled}
        type="button"
        aria-current={activeView === view.id ? 'page' : undefined}
        onclick={() => onNavigate(view.id)}
      >
        {view.label}
      </button>
    </Tooltip>
  {/each}
</nav>
