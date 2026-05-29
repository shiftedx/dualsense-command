<script lang="ts">
  import { ClipboardCopy, Download, ExternalLink } from '@lucide/svelte';

  type SupportBundleBusy = 'copy' | 'download' | '';
  type SupportMessageTone = 'success' | 'info' | 'error';

  export let busy: SupportBundleBusy = '';
  export let message = '';
  export let tone: SupportMessageTone = 'info';
  export let onCopy: () => void | Promise<void> = () => {};
  export let onExport: () => void | Promise<void> = () => {};

  const PROJECT_REPOSITORY_URL = 'https://github.com/shiftedx/dualsense-command';
</script>

<aside id="support-bundle-panel" class="dm-support-panel" aria-label="Support diagnostics bundle">
  <div class="dm-support-copy">
    <span>Support Diagnostics</span>
    <strong>Sanitized support bundle</strong>
    <p>No raw HID paths, raw hardware IDs, serial numbers, or Bluetooth addresses are included.</p>
  </div>
  <div class="dm-support-actions">
    <a class="dm-mini-button" href={PROJECT_REPOSITORY_URL} target="_blank" rel="noreferrer">
      <ExternalLink size={13} /> GitHub
    </a>
    <button class="dm-mini-button" type="button" disabled={Boolean(busy)} onclick={() => void onCopy()}>
      <ClipboardCopy size={13} /> {busy === 'copy' ? 'Copying' : 'Copy JSON'}
    </button>
    <button class="dm-mini-button" type="button" disabled={Boolean(busy)} onclick={() => void onExport()}>
      <Download size={13} /> {busy === 'download' ? 'Exporting' : 'Export JSON'}
    </button>
  </div>
  {#if message}
    <p class:error={tone === 'error'} class:success={tone === 'success'} class="dm-support-message">{message}</p>
  {/if}
</aside>
