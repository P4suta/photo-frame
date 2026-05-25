import { css } from '../../styled-system/css';

// Segmented container — the buttons inside use the
// `segmentedButton` recipe (panda/recipes.ts).
export const segmented = css({
  display: 'grid',
  gridAutoFlow: 'column',
  // `1fr` is a grid-track unit, not a length — sits outside the
  // size-token vocabulary by design.
  gridAutoColumns: '[1fr]',
  border: 'default',
  borderRadius: 'phi.m3',
  overflow: 'hidden',
  background: 'transparent',
});
