import { type JSX, onCleanup, onMount } from 'solid-js';
import { css } from '../styled-system/css';
import { goldenRectangles, K, logSpiralPoint, POLE, RHO0, type Vec2 } from './golden-spiral';

// Background decoration — the inward sibling of `GoldenSpiral`.
//
// The pencil tip now walks the logarithmic spiral *toward* the
// pole (θ decreases with time), and the canvas scale grows in
// lockstep so the tip stays on a fixed on-screen radius. From
// the viewer's perspective the universe expands: rectangles that
// were near the pole swell outward through the tip and off the
// edge of the frame, while ever-smaller rectangles peel off the
// pole behind the tip — the spiral feels like it's being fed
// into the centre forever.
//
// Mechanism is the mirror image of `GoldenSpiral`:
//
//   θ_tip(t) = TH0 - Ω · t     (decreases over time)
//   thR      = θ_tip wrapped to (-2π, 0]
//   scale    = baseScale · exp(-K · thR)   ← grows because thR ≤ 0
//   tipR     = scale · RHO0 · exp(K · thR) ← invariant under the wrap
//
// At the boundary `thR = -2π` the scale has grown by exp(K·2π)
// = φ⁴; wrapping thR back to 0 divides the scale by φ⁴, landing
// on a frame visually identical to the previous one — same
// self-similar seam-free loop as the outward variant.
//
// Rectangle visibility is the mirror condition: a rectangle is
// shown if its *minimum* corner distance from centre is no
// smaller than `tipR / φ⁴`, i.e. up to one full turn *inside*
// the pencil tip. The outward variant uses *max* distance with
// `tipR · φ⁴` to expose targets ahead of an outward-moving tip;
// here the same φⁿ slack exposes the upcoming smaller squares
// the inward-moving tip is heading toward.

const STEPS_OUT = 8;
const STEPS_IN = 60;

// Starting θ — mirror of the outward variant. Begins +2.8π
// outside the steady-state cycle so the intro draws inward from
// the periphery before the wrap-loop kicks in.
const TH0 = 2.8 * Math.PI;

const ANGULAR_PERIOD_S = 9;
const OMEGA = (2 * Math.PI) / ANGULAR_PERIOD_S;

const DTH = 0.022;

const TIP_DOT_R_PX = 2;
const SPIRAL_STROKE_PX = 1.4;
const RECT_STROKE_PX = 1;
// Mirror of the outward `RECT_TIP_SLACK`: show rectangles whose
// nearest corner reaches at least `tipR / φ⁴` so a turn's worth
// of upcoming smaller squares is always visible behind the tip.
const PHI_POW_4 = 6.854_101_966_249_685;
const RECT_TIP_SLACK_INWARD = 1 / PHI_POW_4;
const RECT_MIN_SIDE_PX = 2;
const PENCIL_INK_THRESHOLD_PX = 0.4;

const RTARGET_RATIO = 0.42;

// Reduced-motion static frame anchor — a representative θ that
// sits mid-cycle so a meaningful set of rectangles is rendered.
const STATIC_TH = TH0 - OMEGA * 6;

const TWO_PI = Math.PI * 2;

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
  color: 'fg.dim',
  maskImage: '[radial-gradient(circle at center, black 30%, transparent 62%)]',
  WebkitMaskImage: '[radial-gradient(circle at center, black 30%, transparent 62%)]',
});

const faintProbeCss = css({
  display: 'none',
  color: 'fg.faint',
});

export const GoldenSpiralInward = (): JSX.Element => {
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

    const readColors = (): void => {
      strokeDim = getComputedStyle(canvas).color;
      strokeFaint = getComputedStyle(probe).color;
    };

    const fitCanvas = (): void => {
      const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
      W = canvas.clientWidth;
      H = canvas.clientHeight;
      canvas.width = Math.round(W * dpr);
      canvas.height = Math.round(H * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      cx = W / 2;
      cy = H / 2;
      minDim = Math.min(W, H);
      const Rtarget = minDim * RTARGET_RATIO;
      baseScale = Rtarget / RHO0;
    };

    const drawAt = (thTip: number): void => {
      // Wrap θ into (-2π, 0]. The φ⁴-self-similarity makes the
      // wrap visually continuous — at thR = -2π the scale is
      // exactly φ⁴ larger than at thR = 0, so resetting thR to
      // 0 (and scale back to baseScale) lands on a frame
      // identical to the previous one.
      let thR = thTip > 0 ? thTip : -(-thTip % TWO_PI);
      if (thR > 0) thR = 0;
      const scale = baseScale * Math.exp(-K * thR);
      const tipR = scale * RHO0 * Math.exp(K * thR);

      // Outer sample-clip — beyond this θ the polyline would
      // sweep past the canvas; cap it so the line cost stays
      // bounded. Mirror of `GoldenSpiral`'s inner clip.
      const radiusLimit = minDim * 0.75;
      const thresholdTheta = Math.log(radiusLimit / (scale * RHO0)) / K;
      const thEnd = Math.max(thresholdTheta, thR + 0.15);

      // Inner clip near the pole — sub-pixel ink threshold.
      const innerLimit = PENCIL_INK_THRESHOLD_PX;
      const innerThetaLimit = Math.log(innerLimit / (scale * RHO0)) / K;

      const mx = (p: Vec2): number => cx + scale * (p.x - POLE.x);
      const my = (p: Vec2): number => cy + scale * (p.y - POLE.y);

      ctx.clearRect(0, 0, W, H);

      // Rectangles — revealed where the tip's heading. A
      // rectangle is visible if its *nearest* corner sits inside
      // `tipR` (= the tip's radius times the slack toward the
      // pole). The complementary direction is open: large
      // rectangles outside the tip remain visible as the
      // expanding background, faded by the radial mask.
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
        const minDist = Math.min(...dists);
        if (minDist < tipR * RECT_TIP_SLACK_INWARD || sideLen < RECT_MIN_SIDE_PX) continue;
        ctx.moveTo(px0, py0);
        ctx.lineTo(px1, py1);
        ctx.lineTo(px2, py2);
        ctx.lineTo(px3, py3);
        ctx.closePath();
      }
      ctx.stroke();

      // Logarithmic spiral — from pencil tip (thR) inward to
      // the pole (high θ), trimmed by the sub-pixel limit.
      // Sampling proceeds θ ↑ because the spiral parameterisation
      // grows radius with θ; here we walk *away* from the tip
      // toward smaller-radius interior arcs.
      ctx.strokeStyle = strokeDim;
      ctx.lineWidth = SPIRAL_STROKE_PX;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      ctx.beginPath();
      const pt = logSpiralPoint(thR);
      ctx.moveTo(mx(pt), my(pt));
      // Inward leg: from tip to inner pole limit (θ decreasing
      // into negative territory until the radius is sub-pixel).
      for (let th = thR - DTH; th > innerThetaLimit; th -= DTH) {
        const p = logSpiralPoint(th);
        ctx.lineTo(mx(p), my(p));
      }
      ctx.stroke();

      // Outward trailing leg: rectangles that already passed the
      // tip leave a fading trail of spiral arc behind them as
      // they expand toward the edge. This is what makes the
      // motion read as "the spiral keeps growing outward through
      // the tip" rather than just "the tip slides inward".
      ctx.beginPath();
      ctx.moveTo(mx(pt), my(pt));
      for (let th = thR + DTH; th < thEnd; th += DTH) {
        const p = logSpiralPoint(th);
        ctx.lineTo(mx(p), my(p));
      }
      ctx.stroke();

      // Pencil tip.
      ctx.fillStyle = strokeDim;
      ctx.beginPath();
      ctx.arc(mx(pt), my(pt), TIP_DOT_R_PX, 0, TWO_PI);
      ctx.fill();
    };

    const drawFrame = (now: number): void => {
      if (last === null) last = now;
      const dt = now - last;
      last = now;
      tAcc += dt;
      drawAt(TH0 - OMEGA * (tAcc / 1000));
      rafId = requestAnimationFrame(drawFrame);
    };

    const drawStatic = (): void => {
      drawAt(STATIC_TH);
    };

    const ro = new ResizeObserver(() => {
      fitCanvas();
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
