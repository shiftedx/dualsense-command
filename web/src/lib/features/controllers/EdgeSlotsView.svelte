<script lang="ts">
  import Tooltip from '../../../components/Tooltip.svelte';
  import type {
    ControllerConfiguration,
    ControllerStatus,
    EdgeProfileSlot,
    EdgeProfilesResponse
  } from '../../types';

  export let controller: ControllerStatus | undefined = undefined;
  export let currentControllerConfig: ControllerConfiguration | null = null;
  export let edgeProfiles: EdgeProfilesResponse | null = null;
  export let edgeProfilesLoading = false;
  export let edgeProfilesBusySlot = '';
  export let edgeProfilesError = '';
  export let edgeSlotsReadTooltip = '';
  export let edgeSlotWriteLabel = 'Write';
  export let onRefreshEdgeProfiles: () => void | Promise<void> = () => {};
  export let onWriteEdgeSlot: (slot: EdgeProfileSlot) => void | Promise<void> = () => {};
  export let edgeSlotName: (slot: EdgeProfileSlot) => string = (slot) => slot.name ?? slot.shortcut;
  export let edgeSlotStatus: (slot: EdgeProfileSlot) => string = (slot) => slot.state;
  export let edgeSlotInfoTooltip: (slot: EdgeProfileSlot) => string = (slot) => edgeSlotStatus(slot);
  export let edgeSlotWriteTooltip: (slot: EdgeProfileSlot) => string = (slot) => `Write ${edgeSlotName(slot)}`;

  $: alias = controller?.name || controller?.family || 'No controller';
</script>

<section class="ctl-view" aria-label="Edge onboard slots">
  <div class="ctl-head">
    <h1 class="ctl-title">Edge onboard slots</h1>
    <span class="ctl-sub">{alias} &middot; profiles stored on the controller itself, for checking, not for everyday tuning</span>
  </div>

  <div class="ctl-groups">
    <div class="ctl-group">
      <div class="ctl-group-head">
        <div class="lbl">Onboard memory</div>
        <Tooltip text={edgeSlotsReadTooltip} side="bottom" align="end">
          <button
            type="button"
            class="ctl-button"
            disabled={edgeProfilesLoading}
            aria-label="Refresh DualSense Edge onboard slots"
            onclick={() => void onRefreshEdgeProfiles()}
          >
            {edgeProfilesLoading ? 'Reading' : 'Read'}
          </button>
        </Tooltip>
      </div>

      {#if edgeProfilesError}
        <p class="ctl-note error">{edgeProfilesError}</p>
      {:else if edgeProfiles?.warning}
        <p class="ctl-note">{edgeProfiles.warning}</p>
      {/if}

      <div class="edge-slot-list">
        {#if edgeProfiles?.slots.length}
          {#each edgeProfiles.slots as slot (slot.slotId)}
            <div class="edge-slot-row" class:disabled={!slot.editable}>
              <Tooltip block text={edgeSlotInfoTooltip(slot)} side="right" align="start">
                <div class="edge-slot-copy">
                  <span class="lbl">{slot.shortcut}</span>
                  <strong>{edgeSlotName(slot)}</strong>
                  <small>{edgeSlotStatus(slot)}</small>
                </div>
              </Tooltip>
              {#if slot.editable}
                <Tooltip text={edgeSlotWriteTooltip(slot)} side="left" align="center">
                  <button
                    type="button"
                    class="ctl-button primary"
                    disabled={!currentControllerConfig || edgeProfilesBusySlot === slot.slotId}
                    onclick={() => void onWriteEdgeSlot(slot)}
                  >
                    {edgeProfilesBusySlot === slot.slotId ? 'Writing' : edgeSlotWriteLabel}
                  </button>
                </Tooltip>
              {/if}
            </div>
          {/each}
        {:else}
          <div class="edge-slot-row disabled">
            <div class="edge-slot-copy">
              <span class="lbl">Fn Slots</span>
              <strong>{edgeProfilesLoading ? 'Reading slots' : 'No slot data'}</strong>
              <small>{edgeProfilesLoading ? 'controller scan' : 'unavailable'}</small>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</section>
