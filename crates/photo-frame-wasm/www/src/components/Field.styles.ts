import { css } from '../../styled-system/css';

export const field = css({
  display: 'flex',
  flexDirection: 'column',
  gap: 'phi.m2',
});

export const fieldLabel = css({
  fontSize: 'caption',
  textTransform: 'uppercase',
  letterSpacing: 'caps',
  color: 'fg.dim',
  fontWeight: 'medium',
});

export const fieldBody = css({ display: 'block' });
