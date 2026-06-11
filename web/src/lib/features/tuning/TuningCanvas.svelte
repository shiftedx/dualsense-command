<!--
  Tuning canvas — semantic columns (layout truth: tuning-canvas-v9 mockup).
  Columns group by WHAT IS BEING TUNED (Brake / Throttle / Road feel / Lights),
  never by control type. Extra width grows the trigger-curve instruments only;
  small columns and sliders keep intrinsic caps so no region stretches dead.

  Task 7: `.canvas-grid` lives inside the `.work-and-rail` flex wrapper with
  the saved rail (slot "rail") docked beside it. The rail is furniture — fixed
  width, sticky, outside the column wrap; only the grid reflows.

  The `below` slot parks content that has not been re-homed yet (Tasks 8-10
  move it); nothing previously rendered may be lost.
-->

<section class="tuning-canvas" aria-label="Tuning canvas">
  <div class="work-and-rail">
    <div class="canvas-grid">
      <section class="col-trigger" aria-label="Brake tuning">
        <div class="lbl">Brake · L2</div>
        <div class="canvas-col-body">
          <slot name="brake" />
        </div>
      </section>

      <section class="col-trigger" aria-label="Throttle tuning">
        <div class="lbl">Throttle · R2</div>
        <div class="canvas-col-body">
          <slot name="throttle" />
        </div>
      </section>

      <section class="col-small" aria-label="Road feel tuning">
        <div class="lbl">Road feel</div>
        <div class="canvas-col-body">
          <slot name="road" />
        </div>
        <p class="canvas-hint">More effects appear here for games that send them.</p>
      </section>

      <section class="col-small" aria-label="Lights tuning">
        <div class="lbl">Lights</div>
        <div class="canvas-col-body">
          <slot name="lights" />
        </div>
      </section>
    </div>

    <slot name="rail" />
  </div>

  {#if $$slots.below}
    <div class="canvas-below">
      <slot name="below" />
    </div>
  {/if}
</section>
