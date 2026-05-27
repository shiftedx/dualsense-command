<script lang="ts">
  type ToastTone = 'success' | 'info' | 'error';
  type ToastMessage = {
    id: number;
    tone: ToastTone;
    message: string;
  };

  export let messages: ToastMessage[] = [];
  export let onDismiss: (id: number) => void = () => {};

  const toneLabel = (tone: ToastTone) => {
    if (tone === 'success') return 'Saved';
    if (tone === 'error') return 'Needs attention';
    return 'Notice';
  };
</script>

{#if messages.length}
  <div class="dm-toast-stack" aria-live="polite" aria-atomic="false">
    {#each messages as toast (toast.id)}
      <button class="dm-toast {toast.tone}" type="button" onclick={() => onDismiss(toast.id)}>
        <span>{toneLabel(toast.tone)}</span>
        <strong>{toast.message}</strong>
      </button>
    {/each}
  </div>
{/if}
