<script lang="ts">
  import {
    Check,
    ChevronLeft,
    ChevronRight,
    Gamepad2,
    LifeBuoy,
    RadioTower,
    SlidersHorizontal,
    X
  } from '@lucide/svelte';

  export let open = false;
  export let onClose: () => void = () => {};
  export let onNavigate: (view: 'games' | 'controllers' | 'haptics' | 'buttonMapping') => void = () => {};

  type TutorialStep = {
    title: string;
    eyebrow: string;
    body: string;
    actionLabel: string;
    targetView?: 'games' | 'controllers' | 'haptics' | 'buttonMapping';
    icon: typeof Gamepad2;
  };

  const steps: TutorialStep[] = [
    {
      eyebrow: 'Start here',
      title: 'Pick the controller and scope',
      body: 'Use Controllers for hardware state and live inputs, then use Profiles to choose Global or a supported game scope.',
      actionLabel: 'Open Controllers',
      targetView: 'controllers',
      icon: Gamepad2
    },
    {
      eyebrow: 'Safety first',
      title: 'Game effects wait for telemetry',
      body: 'DSCC keeps triggers neutral until a supported game is detected and fresh telemetry is flowing. Manual test buttons attach only for the short test window.',
      actionLabel: 'Open haptics',
      targetView: 'haptics',
      icon: RadioTower
    },
    {
      eyebrow: 'Tune feel',
      title: 'Shape L2 and R2 with curve points',
      body: 'Drag the trigger dots for custom brake and throttle response, then use Test Actuation to feel the current profile without starting a game.',
      actionLabel: 'Open haptics',
      targetView: 'haptics',
      icon: SlidersHorizontal
    },
    {
      eyebrow: 'Get help',
      title: 'Support bundles stay sanitized',
      body: 'The Support panel copies a diagnostic bundle that leaves out raw HID paths, serials, Bluetooth addresses, and private Steam account paths.',
      actionLabel: 'Stay here',
      icon: LifeBuoy
    }
  ];

  let currentStep = 0;
  let wasOpen = false;

  $: if (open && !wasOpen) {
    currentStep = 0;
    wasOpen = true;
  } else if (!open && wasOpen) {
    wasOpen = false;
  }

  $: activeStep = steps[Math.min(currentStep, steps.length - 1)] ?? steps[0];
  $: atStart = currentStep <= 0;
  $: atEnd = currentStep >= steps.length - 1;

  function close() {
    onClose();
  }

  function previous() {
    currentStep = Math.max(0, currentStep - 1);
  }

  function next() {
    if (atEnd) {
      close();
      return;
    }
    currentStep += 1;
  }

  function openStepTarget() {
    if (activeStep.targetView) onNavigate(activeStep.targetView);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!open || event.key !== 'Escape') return;
    event.preventDefault();
    close();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div class="dm-onboarding" role="dialog" aria-modal="false" aria-label="Quick start guide">
    <div class="dm-onboarding-head">
      <div class="dm-onboarding-icon" aria-hidden="true">
        <svelte:component this={activeStep.icon} size={18} />
      </div>
      <div>
        <span>{activeStep.eyebrow}</span>
        <strong>{activeStep.title}</strong>
      </div>
      <button class="dm-onboarding-close" type="button" aria-label="Skip quick start guide" onclick={close}>
        <X size={15} />
      </button>
    </div>

    <p>{activeStep.body}</p>

    <div class="dm-onboarding-progress" aria-label="Tutorial progress">
      {#each steps as step, index (step.title)}
        <button
          type="button"
          class:active={index === currentStep}
          aria-label={`Go to tutorial step ${index + 1}: ${step.title}`}
          onclick={() => { currentStep = index; }}
        ></button>
      {/each}
    </div>

    <div class="dm-onboarding-actions">
      <button class="dm-onboarding-link" type="button" onclick={close}>Skip</button>
      <div>
        <button class="dm-onboarding-nav" type="button" disabled={atStart} aria-label="Previous tutorial step" onclick={previous}>
          <ChevronLeft size={14} />
        </button>
        {#if activeStep.targetView}
          <button class="dm-onboarding-target" type="button" onclick={openStepTarget}>{activeStep.actionLabel}</button>
        {/if}
        <button class="dm-onboarding-next" type="button" onclick={next}>
          {#if atEnd}
            <Check size={14} /> Done
          {:else}
            Next <ChevronRight size={14} />
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .dm-onboarding {
    position: fixed;
    right: clamp(14px, 2.2vw, 34px);
    bottom: clamp(14px, 2.2vw, 30px);
    z-index: 9800;
    display: grid;
    gap: 13px;
    width: min(410px, calc(100vw - 28px));
    padding: 14px;
    border: 1px solid rgba(0, 112, 204, 0.34);
    border-radius: 8px;
    color: #E2E8F0;
    background: rgba(18, 18, 20, 0.97);
    box-shadow:
      0 22px 60px rgba(0, 0, 0, 0.52),
      inset 0 1px 0 rgba(226, 232, 240, 0.06);
  }

  .dm-onboarding-head {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
  }

  .dm-onboarding-icon {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    border: 1px solid rgba(0, 112, 204, 0.44);
    border-radius: 7px;
    color: #FFFFFF;
    background: rgba(0, 112, 204, 0.18);
  }

  .dm-onboarding-head span {
    display: block;
    color: #3BAEFF;
    font-size: 10px;
    font-weight: 850;
    letter-spacing: 0.12em;
    line-height: 1;
    text-transform: uppercase;
  }

  .dm-onboarding-head strong {
    display: block;
    margin-top: 4px;
    overflow-wrap: anywhere;
    color: #FFFFFF;
    font-size: 16px;
    font-weight: 800;
    line-height: 1.15;
  }

  .dm-onboarding-close,
  .dm-onboarding-nav {
    display: inline-grid;
    width: 30px;
    height: 30px;
    place-items: center;
    border: 1px solid rgba(113, 113, 122, 0.4);
    border-radius: 6px;
    color: #E2E8F0;
    background: rgba(10, 10, 12, 0.42);
  }

  .dm-onboarding-close:hover,
  .dm-onboarding-nav:hover:not(:disabled) {
    border-color: rgba(0, 112, 204, 0.72);
    background: rgba(0, 112, 204, 0.12);
  }

  .dm-onboarding p {
    margin: 0;
    color: rgba(226, 232, 240, 0.78);
    font-size: 13px;
    line-height: 1.5;
  }

  .dm-onboarding-progress {
    display: flex;
    gap: 6px;
  }

  .dm-onboarding-progress button {
    width: 26px;
    height: 4px;
    padding: 0;
    border: 0;
    border-radius: 999px;
    background: rgba(113, 113, 122, 0.34);
  }

  .dm-onboarding-progress button.active {
    background: #3BAEFF;
    box-shadow: 0 0 16px rgba(0, 112, 204, 0.45);
  }

  .dm-onboarding-actions,
  .dm-onboarding-actions > div {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dm-onboarding-actions {
    justify-content: space-between;
  }

  .dm-onboarding-link,
  .dm-onboarding-target,
  .dm-onboarding-next {
    min-height: 30px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 800;
  }

  .dm-onboarding-link {
    border: 0;
    color: rgba(226, 232, 240, 0.68);
    background: transparent;
  }

  .dm-onboarding-link:hover {
    color: #FFFFFF;
  }

  .dm-onboarding-target {
    border: 1px solid rgba(113, 113, 122, 0.46);
    color: #E2E8F0;
    background: rgba(10, 10, 12, 0.42);
  }

  .dm-onboarding-next {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 1px solid rgba(0, 112, 204, 0.92);
    color: #FFFFFF;
    background: #0070CC;
  }

  @media (max-width: 720px) {
    .dm-onboarding {
      left: 14px;
      right: 14px;
      bottom: 14px;
      width: auto;
    }

    .dm-onboarding-actions,
    .dm-onboarding-actions > div {
      align-items: stretch;
    }

    .dm-onboarding-actions {
      display: grid;
    }
  }
</style>
