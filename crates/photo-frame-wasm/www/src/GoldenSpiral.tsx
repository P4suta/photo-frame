import { type JSX, onCleanup, onMount } from 'solid-js';
import { css } from '../styled-system/css';
import {
  CHORD_VISIBLE_STEPS,
  chordSegment,
  chordStateAt,
  goldenRectangles,
  K,
  logSpiralPoint,
  POLE,
  RHO0,
  type Vec2,
} from './golden-spiral';

// Background decoration for the empty drop-zone state.
//
// A "pencil" tip rides the true logarithmic spiral outward from
// the pole forever, leaving a stroke behind it. To keep the tip
// roughly fixed on the same screen radius while the spiral keeps
// growing behind it, the canvas scale shrinks in lockstep with
// the tip's spiral-frame radius: `scale = baseScale·exp(-k·θ)`.
// At θ = 2π the scale has shrunk by `exp(-k·2π) = φ⁻⁴`, so we
// wrap θ back to 0 — and because the spiral is φ⁴-self-similar
// about the pole, the wrapped frame is visually identical to
// the unwrapped one. No seam.
//
// Nested φ-rectangles appear as the pencil reaches them: a
// rectangle is drawn only once its outermost corner is inside
// the tip's current spiral radius (with a small slack so it
// fades into existence rather than popping). Rectangles whose
// on-canvas side has fallen below 2 px are skipped — they would
// alias into a single pixel anyway.
//
// A radial CSS mask vignettes the edges, hiding the (already
// continuous) seam and softening rectangles that swim into and
// out of frame at the periphery.
//
// `prefers-reduced-motion`: the rAF loop is replaced with a
// single static frame at a representative mid-cycle θ. OS-level
// changes to the reduce-motion / colour-scheme preferences are
// hooked so the canvas updates without a reload.

const STEPS_OUT = 8;
const STEPS_IN = 60;

// Starting θ — the pencil enters from the pole as a sub-pixel
// dot (at θ = -7π the radius is RHO0 · φ⁻¹⁴ ≈ RHO0 · 0.00086,
// well below 1 px on any realistic canvas size) and grows out
// of the centre over the first ~8 s before it becomes visibly
// distinct, hitting the on-screen target radius at θ = 0
// (≈ 31.5 s) where the steady-state self-similar loop begins.
const TH0 = -7 * Math.PI;

// One full outward turn (2π) takes ANGULAR_PERIOD_S seconds. 9 s
// matches the reference HTML's tempo — fast enough that the
// pencil never feels stationary, slow enough that you can watch
// individual rectangle corners pass under the tip.
const ANGULAR_PERIOD_S = 9;
const OMEGA = (2 * Math.PI) / ANGULAR_PERIOD_S;

// Angular sampling for the polyline approximation of the spiral.
// 0.022 rad ≈ 1.26°, which is sub-pixel at the typical tip
// radius (~0.42 · minDim) and small enough that the polyline
// reads as a smooth curve under `lineJoin: round`.
const DTH = 0.022;

// Rendering constants — chosen to match photo-frame's stroke-
// only monochrome language. The reference HTML's gold + glow
// + hot-trail are intentionally removed; the spiral itself is
// the focal point, not the rendering effects.
const TIP_DOT_R_PX = 2;
const SPIRAL_STROKE_PX = 1.4;
const RECT_STROKE_PX = 1;
// Show rectangles out to one full turn ahead of the pencil tip
// (the spiral grows by exactly φ⁴ per 2π turn, so multiplying
// the tip radius by φ⁴ keeps roughly one turn's worth of
// upcoming vertices visible). This lets the viewer read the
// next vertex as a target the pencil is heading toward, instead
// of only seeing the rectangles after they've already been
// traced. The 1.06 used in the reference HTML hides everything
// past the tip — that's the wrong default for the empty-state
// decoration, where the upcoming vertices *are* the point.
const PHI_POW_4 = 6.854_101_966_249_685;
const RECT_TIP_SLACK = PHI_POW_4;
const RECT_MIN_SIDE_PX = 2; // below this, the rect would alias
const PENCIL_INK_THRESHOLD_PX = 0.4; // inner stroke clip (anti-aliased
// near-pole limit)

// Pencil tip sits at this fraction of the smaller canvas
// dimension. 0.42 keeps the tip clear of both the centre and
// the vignette band.
const RTARGET_RATIO = 0.42;

const TWO_PI = Math.PI * 2;
const HALF_PI = Math.PI / 2;

// Reduced-motion static frame anchor — pick a θ a couple of
// turns past the pencil-becomes-visible point so rectangles are
// populated and the tip sits in a representative on-screen
// position (steady-state, not the sub-pixel intro phase).
const STATIC_TH = HALF_PI * 3;

// Below this on-canvas chord length (px), a chord is treated as
// "central" — the inward pole-region chords shrink toward the
// outward direction (V_b side) instead of the usual V_a side,
// so they feel like they're being absorbed by the outward
// motion rather than peeling backward.
const CHORD_CENTRAL_PX = 24;

// Vertex ripple — every rectangle corner sits on the spiral at
// θ = n·π/2, so a small ring rendered when the pencil tip
// crosses one of those θ values lands exactly on the corner
// without any extra geometry lookup. Lifetime is far shorter
// than the 2π wrap (9 s) so the φ⁴ self-similar reset never
// catches a ripple mid-life.
const RIPPLE_LIFETIME_MS = 700;
const RIPPLE_MAX_R_PX = 12;
const RIPPLE_STROKE_PX = 1;

const wrapperCss = css({
  position: 'absolute',
  inset: '0',
  width: 'full',
  height: 'full',
  pointerEvents: 'none',
  zIndex: '[0]',
  overflow: 'hidden',
});

const canvasCss = css({
  display: 'block',
  width: 'full',
  height: 'full',
  // `color` is what `getComputedStyle().color` reads back so the
  // canvas can paint with the theme's `fg.dim` rgb string — the
  // canvas has no text, so this style only exists as a probe.
  color: 'fg.dim',
  // Radial vignette via CSS mask so the canvas itself never
  // needs to know the page background; only opaque pixels reach
  // the eye where the mask is opaque. Matches the reference
  // HTML's `BG·fillRect + radial gradient` trick but moves the
  // crossfade off the render loop's hot path.
  maskImage: '[radial-gradient(circle at center, black 30%, transparent 62%)]',
  WebkitMaskImage: '[radial-gradient(circle at center, black 30%, transparent 62%)]',
});

// Probe span — `display: none`, present only so we can read
// `fg.faint` off it via `getComputedStyle`. Panda hashes its CSS
// variable names, so there's no stable `--colors-fg-faint` to
// query directly.
const faintProbeCss = css({
  display: 'none',
  color: 'fg.faint',
});

export const GoldenSpiral = (): JSX.Element => {
  let canvasRef: HTMLCanvasElement | undefined;
  let faintProbeRef: HTMLSpanElement | undefined;

  onMount(() => {
    const canvas = canvasRef;
    const probe = faintProbeRef;
    if (!canvas || !probe) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const rects = goldenRectangles(STEPS_OUT, STEPS_IN);

    let W = 0;
    let H = 0;
    let cx = 0;
    let cy = 0;
    let minDim = 0;
    let baseScale = 1;
    let strokeDim = '';
    let strokeFaint = '';
    let rafId = 0;
    let last: number | null = null;
    let tAcc = 0;

    // Active vertex ripples and the previous frame's pencil θ
    // for crossing detection. `thetaSpiral` is the pole-frame θ
    // (= n · π/2) so its on-canvas position rides the scale
    // shrink along with the rest of the spiral — the ripple
    // appears at the corner and drifts inward with the rest of
    // the imagery, never floating off as a screen-fixed artifact.
    type Ripple = { thetaSpiral: number; t0: number };
    const ripples: Ripple[] = [];
    let prevThTip: number | null = null;

    const readColors = (): void => {
      strokeDim = getComputedStyle(canvas).color;
      strokeFaint = getComputedStyle(probe).color;
    };

    // Named `fitCanvas` rather than `fit` because biome's
    // `noFocusedTests` rule treats bare `fit()` calls as Jasmine
    // focused tests and flags every call site.
    const fitCanvas = (): void => {
      const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
      W = canvas.clientWidth;
      H = canvas.clientHeight;
      canvas.width = Math.round(W * dpr);
      canvas.height = Math.round(H * dpr);
      // Author in CSS-pixel units; the DPR scaling is on the
      // transform so we can call `clientWidth`-based maths
      // throughout the draw routine without touching DPR again.
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      cx = W / 2;
      cy = H / 2;
      minDim = Math.min(W, H);
      const Rtarget = minDim * RTARGET_RATIO;
      baseScale = Rtarget / RHO0;
    };

    const drawAt = (thTip: number, now: number): void => {
      // Vertex crossing detection — every n·π/2 the pencil
      // sweeps past is a corner contact. Push one ripple per
      // crossing into the active list; the list is drained
      // below as each ripple ages out. `prevThTip` is null on
      // the first frame (and after a reduced-motion pause/
      // resume) so we skip detection until we have a baseline.
      if (prevThTip !== null && thTip > prevThTip) {
        const nStart = Math.floor(prevThTip / HALF_PI) + 1;
        const nEnd = Math.floor(thTip / HALF_PI);
        for (let n = nStart; n <= nEnd; n++) {
          ripples.push({ thetaSpiral: n * HALF_PI, t0: now });
        }
      }
      prevThTip = thTip;

      // Wrap θ by 2π only after we leave the intro phase (thTip
      // negative). The φ⁴-self-similarity makes the wrap
      // visually continuous.
      const thR = thTip < 0 ? thTip : thTip % TWO_PI;
      const scale = baseScale * Math.exp(-K * Math.max(0, thR));
      const tipR = scale * RHO0 * Math.exp(K * thR);

      // Inner sample-start — clip the polyline at the θ where
      // its spiral radius would fall below the ink-threshold
      // sub-pixel width. The `thR - 0.15` floor guards against
      // numerical blow-up when the threshold is already past
      // the tip itself (early intro frames).
      const thresholdTheta = Math.log(PENCIL_INK_THRESHOLD_PX / (scale * RHO0)) / K;
      const thStart = Math.min(thresholdTheta, thR - 0.15);

      const mx = (p: Vec2): number => cx + scale * (p.x - POLE.x);
      const my = (p: Vec2): number => cy + scale * (p.y - POLE.y);

      ctx.clearRect(0, 0, W, H);

      // Rectangles — revealed once the pencil tip reaches them.
      ctx.strokeStyle = strokeFaint;
      ctx.lineWidth = RECT_STROKE_PX;
      ctx.beginPath();
      for (const q of rects) {
        const px0 = mx(q[0]);
        const py0 = my(q[0]);
        const px1 = mx(q[1]);
        const py1 = my(q[1]);
        const px2 = mx(q[2]);
        const py2 = my(q[2]);
        const px3 = mx(q[3]);
        const py3 = my(q[3]);
        const sideLen = Math.hypot(px1 - px0, py1 - py0);
        const dists = [
          Math.hypot(px0 - cx, py0 - cy),
          Math.hypot(px1 - cx, py1 - cy),
          Math.hypot(px2 - cx, py2 - cy),
          Math.hypot(px3 - cx, py3 - cy),
        ];
        const maxDist = Math.max(...dists);
        if (maxDist > tipR * RECT_TIP_SLACK || sideLen < RECT_MIN_SIDE_PX) continue;
        ctx.moveTo(px0, py0);
        ctx.lineTo(px1, py1);
        ctx.lineTo(px2, py2);
        ctx.lineTo(px3, py3);
        ctx.closePath();
      }
      ctx.stroke();

      // Logarithmic spiral, from inner sample-start out to tip.
      ctx.strokeStyle = strokeDim;
      ctx.lineWidth = SPIRAL_STROKE_PX;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      ctx.beginPath();
      const p0 = logSpiralPoint(thStart);
      ctx.moveTo(mx(p0), my(p0));
      for (let th = thStart + DTH; th < thR; th += DTH) {
        const p = logSpiralPoint(th);
        ctx.lineTo(mx(p), my(p));
      }
      const pt = logSpiralPoint(thR);
      ctx.lineTo(mx(pt), my(pt));
      ctx.stroke();

      // Chords — connect each pair of consecutive spiral
      // vertices (V_n, V_{n+1}) with a straight segment that
      // grows out of V_n as the pencil approaches V_{n+1},
      // holds at full length for `CHORD_VISIBLE_STEPS` further
      // quarter-turns, then retracts. Each chord is the
      // diagonal of one nested rectangle (V_n and V_{n+1} are
      // opposite corners). Central chords near the pole —
      // those whose on-canvas length has fallen below
      // `CHORD_CENTRAL_PX` — flip their shrink direction so
      // they're absorbed outward (V_b side) rather than peeled
      // back from V_a, matching the inward-scaling motion the
      // central region is under. Iteration bounds are tight to
      // the active window: only the at-most `visibleSteps + 2`
      // chords that could possibly be visible.
      ctx.strokeStyle = strokeFaint;
      ctx.lineWidth = RECT_STROKE_PX;
      ctx.beginPath();
      const chordIndexMax = Math.floor(thR / HALF_PI);
      const chordIndexMin = chordIndexMax - CHORD_VISIBLE_STEPS - 1;
      for (let n = chordIndexMin; n <= chordIndexMax; n++) {
        const thetaA = n * HALF_PI;
        const state = chordStateAt(thetaA, thR);
        if (state.phase === 'hidden') continue;
        const va = logSpiralPoint(thetaA);
        const vb = logSpiralPoint(thetaA + HALF_PI);
        const chordPx = scale * Math.hypot(vb.x - va.x, vb.y - va.y);
        const flip = chordPx < CHORD_CENTRAL_PX;
        const seg = chordSegment(thetaA, state, flip);
        if (!seg) continue;
        ctx.moveTo(mx(seg.start), my(seg.start));
        ctx.lineTo(mx(seg.end), my(seg.end));
      }
      ctx.stroke();

      // Pencil tip — a small filled dot in the same ink colour
      // so the line reads as "actively being drawn", not just
      // "ends here".
      ctx.fillStyle = strokeDim;
      ctx.beginPath();
      ctx.arc(mx(pt), my(pt), TIP_DOT_R_PX, 0, TWO_PI);
      ctx.fill();

      // Vertex ripples — circular blips at the corners the
      // pencil has just passed. Each ripple grows from 0 →
      // RIPPLE_MAX_R_PX and fades 1 → 0 over its lifetime,
      // pinned to its corner in spiral coordinates so it
      // drifts inward with the rest of the imagery.
      ctx.strokeStyle = strokeDim;
      ctx.lineWidth = RIPPLE_STROKE_PX;
      for (let i = ripples.length - 1; i >= 0; i--) {
        const r = ripples[i];
        if (!r) continue;
        const age = (now - r.t0) / RIPPLE_LIFETIME_MS;
        if (age >= 1) {
          ripples.splice(i, 1);
          continue;
        }
        const p = logSpiralPoint(r.thetaSpiral);
        ctx.globalAlpha = 1 - age;
        ctx.beginPath();
        ctx.arc(mx(p), my(p), age * RIPPLE_MAX_R_PX, 0, TWO_PI);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
    };

    const drawFrame = (now: number): void => {
      if (last === null) last = now;
      const dt = now - last;
      last = now;
      tAcc += dt;
      drawAt(TH0 + OMEGA * (tAcc / 1000), now);
      rafId = requestAnimationFrame(drawFrame);
    };

    const drawStatic = (): void => {
      // Reset the crossing baseline so a static repaint at a
      // different θ doesn't dump a burst of ripples into the
      // queue (which would then visibly fade in over the static
      // frame and contradict the "no motion" contract).
      prevThTip = STATIC_TH;
      ripples.length = 0;
      drawAt(STATIC_TH, performance.now());
    };

    const ro = new ResizeObserver(() => {
      fitCanvas();
      // When paused (reduced-motion), redraw immediately at the
      // new size; otherwise the next rAF tick will repaint.
      if (rafId === 0) drawStatic();
    });

    const mqL = window.matchMedia('(prefers-color-scheme: light)');
    const mqR = window.matchMedia('(prefers-reduced-motion: reduce)');

    const onColorSchemeChange = (): void => {
      readColors();
      if (rafId === 0) drawStatic();
    };
    const onReducedMotionChange = (e: MediaQueryListEvent): void => {
      if (e.matches) {
        if (rafId !== 0) {
          cancelAnimationFrame(rafId);
          rafId = 0;
        }
        drawStatic();
      } else if (rafId === 0) {
        // Reset `last` so the first dt after resuming is small,
        // not the wall-clock elapsed since pause.
        last = null;
        rafId = requestAnimationFrame(drawFrame);
      }
    };

    readColors();
    fitCanvas();
    ro.observe(canvas);
    mqL.addEventListener('change', onColorSchemeChange);
    mqR.addEventListener('change', onReducedMotionChange);

    if (mqR.matches) {
      drawStatic();
    } else {
      rafId = requestAnimationFrame(drawFrame);
    }

    onCleanup(() => {
      if (rafId !== 0) cancelAnimationFrame(rafId);
      ro.disconnect();
      mqL.removeEventListener('change', onColorSchemeChange);
      mqR.removeEventListener('change', onReducedMotionChange);
    });
  });

  return (
    <div class={wrapperCss} aria-hidden="true">
      <span
        ref={(el) => {
          faintProbeRef = el;
        }}
        class={faintProbeCss}
      />
      <canvas
        ref={(el) => {
          canvasRef = el;
        }}
        class={canvasCss}
      />
    </div>
  );
};
