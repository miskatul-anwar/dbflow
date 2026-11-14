import '@fontsource/inter/400.css';
import '@fontsource/inter/500.css';
import '@fontsource/inter/600.css';
import '@fontsource/inter/700.css';
import '@xyflow/react/dist/style.css';

import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles/theme.css';
import { initTheme } from './theme/theme';
import { initEngine } from './engine';

// Set data-theme before the first render so there is no flash of the wrong theme.
initTheme();

const root = ReactDOM.createRoot(document.getElementById('root')!);
const render = () =>
  root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>
  );

// The Rust engine (WebAssembly build) serves the small synchronous calls the UI makes (diagram
// builders, relation signatures); full runs use the native engine in the desktop app.
initEngine().then(render, err => {
  console.error('Failed to load the DBFlow engine', err);
  render();
});
