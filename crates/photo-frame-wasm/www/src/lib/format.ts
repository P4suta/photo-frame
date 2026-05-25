// Trivial format / utility helpers shared across the UI. Kept
// here (= `src/lib/format.ts`) rather than scattered through
// `App.tsx` / `Gallery.tsx` so their behaviour is unit-testable
// without spinning up the SolidJS reactive scope.

/** Insert `_framed` before the file extension. If the original
 *  has no extension the suffix is appended directly. The output
 *  is always `.jpg` regardless of source extension because the
 *  pipeline always encodes JPEG. */
export const framedName = (original: string): string => {
  const dot = original.lastIndexOf('.');
  const stem = dot >= 0 ? original.slice(0, dot) : original;
  return `${stem}_framed.jpg`;
};

/** Best-effort textification of an unknown thrown value — the
 *  `try { ... } catch (e: unknown) { setStatus(stringifyError(e)) }`
 *  pattern. Native `Error` keeps its `.message`, primitives go
 *  through `String(...)`, and exotic objects fall back to
 *  `String(value)` rather than `[object Object]`. */
export const stringifyError = (error: unknown): string => {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  return String(error);
};

/** Copy the contents of a typed-array view into a *fresh*
 *  `ArrayBuffer`. `Blob`s want a real `ArrayBuffer` (or
 *  `SharedArrayBuffer`) — passing a view directly tries to
 *  share the underlying buffer, which crashes when the view's
 *  buffer is `SharedArrayBuffer`. Always returning a regular
 *  `ArrayBuffer` sidesteps that whole class of bug. */
export const uint8ToBuffer = (u8: Uint8Array): ArrayBuffer => {
  const buffer = new ArrayBuffer(u8.byteLength);
  new Uint8Array(buffer).set(u8);
  return buffer;
};

/** Gallery row status → short user-readable caption. Exhaustive
 *  switch (the union has four members); TS will flag any new
 *  status that gets added to the union but not handled here. */
export type RowStatus = 'queued' | 'processing' | 'done' | 'error';
export const statusLabel = (status: RowStatus): string => {
  switch (status) {
    case 'queued':
      return 'Queued';
    case 'processing':
      return 'Processing';
    case 'done':
      return '✓ Done';
    case 'error':
      return '✗ Error';
  }
};
