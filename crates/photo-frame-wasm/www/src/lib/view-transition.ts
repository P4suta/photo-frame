// Thin wrapper over `document.startViewTransition` for the
// stale-while-revalidate UX shared by the single-image preview
// (`CanvasPreview.tsx`) and the batch gallery
// (`Gallery.tsx` via `state/batch-session.ts`).
//
// The View Transitions API is Chrome 111+ / Safari 18+ /
// Firefox 132+. Older engines fall back to a hard cut — combined
// with the project's existing variant caches the regression
// reads as a "no animation" experience, not a broken one.
//
// Lives outside any component so both call sites speak the same
// vocabulary and so unit tests can stub the global cheaply
// (the function is just a 4-line `if`-check, but pulling it out
// keeps the call sites readable as "swap the DOM with a fade").

type DocVT = Document & {
  startViewTransition?: (callback: () => void) => unknown;
};

/** Run `mutate` inside `document.startViewTransition` when the
 *  current engine supports it; otherwise run it directly. The
 *  callback must perform the entire synchronous DOM mutation
 *  that produces the post-transition state — the browser captures
 *  the post-mutation snapshot immediately after `mutate` returns
 *  and crossfades from the pre-mutation snapshot. */
export const withViewTransition = (mutate: () => void): void => {
  if (typeof document === 'undefined') {
    mutate();
    return;
  }
  const doc = document as DocVT;
  if (typeof doc.startViewTransition === 'function') {
    doc.startViewTransition(mutate);
  } else {
    mutate();
  }
};
