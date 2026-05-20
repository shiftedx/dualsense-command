<script lang="ts">
  export let text = '';
  export let side: 'top' | 'right' | 'bottom' | 'left' = 'top';
  export let align: 'start' | 'center' | 'end' = 'center';
  export let block = false;

  const id = `dscc-tooltip-${Math.random().toString(36).slice(2)}`;
  let hostEl: HTMLSpanElement | undefined;
  let bubbleEl: HTMLSpanElement | undefined;
  let bubbleStyle = '';
  let resolvedSide = side;

  const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

  function updateTooltipPosition() {
    if (!hostEl || !bubbleEl || typeof window === 'undefined') return;

    const gap = 8;
    const margin = 8;
    const host = hostEl.getBoundingClientRect();
    const bubble = bubbleEl.getBoundingClientRect();
    let nextSide = side;

    if (side === 'bottom' && host.bottom + gap + bubble.height > window.innerHeight - margin) nextSide = 'top';
    if (side === 'top' && host.top - gap - bubble.height < margin) nextSide = 'bottom';
    if (side === 'right' && host.right + gap + bubble.width > window.innerWidth - margin) nextSide = 'left';
    if (side === 'left' && host.left - gap - bubble.width < margin) nextSide = 'right';

    let left = host.left;
    let top = host.top;

    if (nextSide === 'top' || nextSide === 'bottom') {
      top = nextSide === 'bottom' ? host.bottom + gap : host.top - gap - bubble.height;
      if (align === 'center') left = host.left + host.width / 2 - bubble.width / 2;
      if (align === 'end') left = host.right - bubble.width;
    } else {
      left = nextSide === 'right' ? host.right + gap : host.left - gap - bubble.width;
      top = host.top + host.height / 2 - bubble.height / 2;
    }

    left = clamp(left, margin, Math.max(margin, window.innerWidth - bubble.width - margin));
    top = clamp(top, margin, Math.max(margin, window.innerHeight - bubble.height - margin));
    resolvedSide = nextSide;
    bubbleStyle = `left:${left}px;top:${top}px;`;
  }
</script>

<span
  bind:this={hostEl}
  class="dscc-tooltip"
  class:block
  data-side={side}
  data-align={align}
  role="presentation"
  aria-describedby={text ? id : undefined}
  onmouseenter={updateTooltipPosition}
  onmousemove={updateTooltipPosition}
  onfocusin={updateTooltipPosition}
>
  <slot />
  {#if text}
    <span
      bind:this={bubbleEl}
      id={id}
      class="dscc-tooltip-bubble"
      data-resolved-side={resolvedSide}
      role="tooltip"
      style={bubbleStyle}
    >{text}</span>
  {/if}
</span>

<style>
  .dscc-tooltip {
    position: relative;
    display: inline-flex;
    align-items: inherit;
    min-width: 0;
  }

  .dscc-tooltip.block {
    display: block;
    width: 100%;
  }

  .dscc-tooltip-bubble {
    position: fixed;
    z-index: 10000;
    display: block;
    visibility: hidden;
    max-width: min(23rem, calc(100vw - 2rem));
    width: max-content;
    padding: 0.58rem 0.68rem;
    border: 1px solid rgba(0, 112, 204, 0.34);
    border-radius: 5px;
    color: #E2E8F0;
    background: rgba(18, 18, 20, 0.98);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.48);
    font-family: Inter, ui-sans-serif, system-ui, sans-serif;
    font-size: 0.72rem;
    font-weight: 650;
    line-height: 1.35;
    letter-spacing: 0;
    text-align: left;
    text-transform: none;
    white-space: normal;
    opacity: 0;
    pointer-events: none;
    transform: translateY(0.2rem);
    transition: opacity 90ms ease, transform 90ms ease, visibility 90ms ease;
  }

  .dscc-tooltip:hover > .dscc-tooltip-bubble,
  .dscc-tooltip:focus-visible > .dscc-tooltip-bubble,
  .dscc-tooltip:focus-within > .dscc-tooltip-bubble {
    visibility: visible;
    opacity: 1;
    transform: translateY(0);
  }
</style>
