<script lang="ts">
  export let active = false;
</script>

<section
  class:dm-view-hidden={!active}
  class="dm-deck"
  aria-label="Adaptive triggers and haptics"
  aria-hidden={!active}
>
  <slot />
</section>
