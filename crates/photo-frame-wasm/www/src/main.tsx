import { render } from 'solid-js/web';
import { App } from './App';
import { loadPresets } from './frame-client';
import './styles/global.css';

const root = document.getElementById('root');
if (!root) throw new Error('#root not found in index.html');

// Resolve the Rust-side preset table once before mounting the shell.
// Awaiting here (rather than letting `<App>` resolve on its own) keeps
// the entire `AppSettings` reactive graph free of a "presets are
// loading" branch — the default preset, the initial PipelineSpec, and
// the segmented control all see the canonical truth on the first
// paint.
const presets = await loadPresets();
render(() => <App presets={presets} />, root);
