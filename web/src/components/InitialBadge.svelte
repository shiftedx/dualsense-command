<script lang="ts">
  // Console-tech placeholder for items missing real artwork. Renders the first
  // letter of `label` inside a rounded card with corner brackets and a subtle
  // glow. Accent color tints the gradient + brackets so callers can express
  // scope (cyan = built-in, green = per-game, purple = global, etc.).
  export let label: string = '?';
  export let accent: string = '#3BA0FF';
  /** Square pixel size of the rendered badge. */
  export let size: number = 36;
  /** Class string forwarded to the root svg for layout/positioning. */
  let className = '';
  export { className as class };

  // IMPORTANT: these IDs MUST be computed once per component instance, not on
  // every render. Putting them in `$:` blocks regenerated them on every signal
  // write — with ~22 badges rendered on the Games tab, that triggered massive
  // SVG def re-link work on every keystroke / click / snapshot push and made
  // unrelated buttons (Show details, etc.) feel laggy.
  const gradientId = `dm-initial-grad-${Math.random().toString(36).slice(2, 10)}`;
  const highlightId = `dm-initial-glow-${Math.random().toString(36).slice(2, 10)}`;

  $: trimmed = (label ?? '').trim();
  $: initial = trimmed
    ? Array.from(trimmed)[0]?.toUpperCase() ?? '?'
    : '?';
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 64 64"
  class={className}
  aria-hidden="true"
  role="img"
>
  <defs>
    <linearGradient id={gradientId} x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color={accent} stop-opacity="0.42" />
      <stop offset="55%" stop-color={accent} stop-opacity="0.18" />
      <stop offset="100%" stop-color={accent} stop-opacity="0.06" />
    </linearGradient>
    <radialGradient id={highlightId} cx="28%" cy="22%" r="56%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.22" />
      <stop offset="70%" stop-color="#FFFFFF" stop-opacity="0" />
    </radialGradient>
  </defs>

  <!-- Card -->
  <rect
    x="2.5"
    y="2.5"
    width="59"
    height="59"
    rx="10"
    ry="10"
    fill={`url(#${gradientId})`}
    stroke={accent}
    stroke-opacity="0.62"
    stroke-width="1.5"
  />
  <rect
    x="2.5"
    y="2.5"
    width="59"
    height="59"
    rx="10"
    ry="10"
    fill={`url(#${highlightId})`}
  />

  <!-- Corner brackets give it the console-instrument feel -->
  <path
    d="M 8 16 L 8 8 L 16 8"
    stroke={accent}
    stroke-opacity="0.92"
    stroke-width="1.6"
    stroke-linecap="round"
    fill="none"
  />
  <path
    d="M 48 56 L 56 56 L 56 48"
    stroke={accent}
    stroke-opacity="0.92"
    stroke-width="1.6"
    stroke-linecap="round"
    fill="none"
  />

  <!-- Status dot, kept low-key so it reads as a stable indicator, not a notification -->
  <circle cx="54" cy="12" r="2" fill={accent} fill-opacity="0.85" />

  <!-- The initial. Space Grotesk is the brand display face used in the app shell. -->
  <text
    x="32"
    y="42"
    text-anchor="middle"
    font-family="Space Grotesk, Inter Tight, Inter, sans-serif"
    font-size="30"
    font-weight="700"
    fill="#FFFFFF"
    letter-spacing="0.02em"
  >{initial}</text>
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
