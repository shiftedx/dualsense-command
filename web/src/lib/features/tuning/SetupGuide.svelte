<script lang="ts">
  import { deriveSetupModel, TELEMETRY_TARGET_IP } from './setupRequirements';
  import type { SupportedGame } from '../../types';

  // Per-game setup walkthrough (Task 8). Rendered as a CANVAS STATE: it
  // replaces the tuning grid until the game's requirements verify, and can be
  // re-entered any time from the telemetry chip or the game dropdown.
  // Verification is passive — the LISTENING box flips green by itself when
  // packets arrive and the canvas swaps back in without a click.

  let {
    game,
    telemetryFresh = false,
    verified = false,
    port = 5300,
    packetRateHz = 0,
    adapterName = '',
    adapterHint = '',
    onVerified = () => {},
    onStartTuning = () => {}
  }: {
    game: SupportedGame;
    telemetryFresh?: boolean;
    verified?: boolean;
    port?: number;
    packetRateHz?: number;
    adapterName?: string;
    adapterHint?: string;
    onVerified?: () => void;
    onStartTuning?: () => void;
  } = $props();

  const model = $derived(deriveSetupModel({ game, telemetryFresh, verified, port }));

  // Passive verification: the first fresh packets complete setup on their
  // own. A short green beat on the LISTENING box, then the parent swaps the
  // tuning canvas in. Manual re-entry (already verified) never auto-closes.
  const VERIFY_SWAP_DELAY_MS = 900;
  $effect(() => {
    if (!telemetryFresh || verified || game.supportLevel !== 'telemetry') return;
    const timer = window.setTimeout(() => onVerified(), VERIFY_SWAP_DELAY_MS);
    return () => window.clearTimeout(timer);
  });

  // Copy buttons: clipboard API first, hidden-textarea fallback (same graceful
  // degradation the support bundle copy uses), calm inline failure note last.
  let copiedValue = $state('');
  let copyFailed = $state(false);
  let copyResetTimer = 0;

  const fallbackCopy = (value: string): boolean => {
    const area = document.createElement('textarea');
    area.value = value;
    area.setAttribute('readonly', '');
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.appendChild(area);
    area.select();
    let ok = false;
    try {
      ok = document.execCommand('copy');
    } catch {
      ok = false;
    }
    area.remove();
    return ok;
  };

  const copyValue = async (value: string) => {
    let ok = false;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(value);
        ok = true;
      } else {
        ok = fallbackCopy(value);
      }
    } catch {
      ok = fallbackCopy(value);
    }
    copiedValue = ok ? value : '';
    copyFailed = !ok;
    window.clearTimeout(copyResetTimer);
    copyResetTimer = window.setTimeout(() => {
      copiedValue = '';
      copyFailed = false;
    }, 1600);
  };

  $effect(() => {
    return () => {
      window.clearTimeout(copyResetTimer);
    };
  });

  const stepNumber = (index: number) => index + 1;
</script>

<section class="setup-guide" aria-label={`Setup guide for ${game.name}`}>
  {#if model.required}
    <header class="setup-guide-head">
      <h2>One-time setup for {game.name}</h2>
      <p class="setup-guide-sub">
        {#if verified}
          Already verified — these are the values DSCC is listening with.
        {:else}
          About 2 minutes, once. Then it's automatic forever.
        {/if}
      </p>
    </header>

    <div class="setup-guide-grid">
      <ol class="setup-steps" role="list">
        {#each model.steps as step, index (step.id)}
          <li class="setup-step" data-state={step.state}>
            <span class="setup-stepnum" class:done={step.state === 'done'} class:now={step.state === 'now'} aria-hidden="true">
              {step.state === 'done' ? '✓' : stepNumber(index)}
            </span>
            <div class="setup-step-body">
              <div class="setup-step-title" class:settled={step.state !== 'now'}>
                {step.title}
                <span class="visually-hidden">
                  {step.state === 'done' ? ' — done' : step.state === 'now' ? ' — current step' : ''}
                </span>
              </div>
              <div class="setup-step-detail">
                {step.detail}{#if step.menuPath}<b>{step.menuPath}</b>{step.detailAfterPath ?? ''}{/if}
              </div>
              {#if step.copyValues?.length}
                <div class="setup-copy-card">
                  {#each step.copyValues as item (item.label)}
                    <div class="setup-copy-row">
                      <span class="setup-copy-label">{item.label}</span>
                      <code class="setup-kbd">{item.value}</code>
                      <button class="setup-copy-button" type="button" onclick={() => void copyValue(item.value)}>
                        {copiedValue === item.value ? 'copied ✓' : 'copy'}
                      </button>
                    </div>
                  {/each}
                  {#if copyFailed}
                    <div class="setup-copy-fallback" role="status">Copy is blocked in this browser — select the value and copy it yourself.</div>
                  {/if}
                </div>
              {/if}
            </div>
          </li>
        {/each}
      </ol>

      <aside class="setup-listening" class:ok={model.fresh} aria-live="polite">
        <span class="setup-lbl">Listening</span>
        <div class="setup-listening-line">
          <span class="setup-listening-dot" aria-hidden="true"></span>
          {#if model.fresh}
            <span>Packets arriving on port {model.port}{packetRateHz > 0 ? ` · ${Math.round(packetRateHz)} Hz` : ''}</span>
          {:else}
            <span>Waiting on port {model.port}…</span>
          {/if}
        </div>
        <p class="setup-listening-note">
          {#if model.fresh && !verified}
            Data arrived — setup is complete. Opening the tuning canvas…
          {:else if model.fresh}
            Telemetry is fresh. The driving feel is live.
          {:else}
            Nothing has arrived yet. That's expected until you finish step 2 and start driving.
            This box turns green by itself — no refresh, no button.
          {/if}
        </p>
        <details class="setup-stuck">
          <summary>Stuck? Common {game.name} fixes</summary>
          <ul>
            <li>Data Out only sends while you're driving — menus stay silent.</li>
            <li>Double-check the in-game values: IP <code class="setup-kbd">{TELEMETRY_TARGET_IP}</code>, port <code class="setup-kbd">{model.port}</code>.</li>
            <li>If another app is using port {model.port}, close it and re-enter a driving session.</li>
            <li>The feed stays on this PC — packets go to {TELEMETRY_TARGET_IP} and never leave your machine.</li>
          </ul>
          {#if adapterHint}
            <p class="setup-adapter-hint">{adapterName ? `${adapterName}: ` : ''}{adapterHint}</p>
          {/if}
        </details>
      </aside>
    </div>
  {:else}
    <header class="setup-guide-head">
      <span class="setup-zero-chip">● No setup needed</span>
      <h2>{game.name} is ready when you are</h2>
      <p class="setup-guide-sub">
        DSCC detects this game on its own. Start the game and your tuned feel loads with it —
        nothing to configure.
      </p>
    </header>
    <div class="setup-pretune">
      <p>Meanwhile, you can pre-tune its profile with base feel — no game required.</p>
      <button class="setup-pretune-button" type="button" onclick={() => void onStartTuning()}>
        Start tuning →
      </button>
    </div>
  {/if}
</section>
