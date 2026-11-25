import '@fontsource/inter/400.css';
import '@fontsource/inter/500.css';
import '@fontsource/inter/600.css';
import '@fontsource/inter/700.css';
import '@xyflow/react/dist/style.css';

import React, { lazy, Suspense } from 'react';
import { MotionConfig } from 'framer-motion';
import { TitleTooltips } from './components/ui/TitleTooltips';
import ReactDOM from 'react-dom/client';
import App from './App';
import { wantsMobileUi } from './utils/platform';
import { modelNamingChosen, setModelNaming } from './services/naming/userNames';
import './styles/theme.css';
import { initTheme } from './theme/theme';
import { initEngine } from './engine';

// Set data-theme before the first render so there is no flash of the wrong theme.
initTheme();

// The phone interface (Android app) is its own bundle; the desktop interface is unchanged.
const MobileApp = lazy(() => import('./mobile/MobileApp'));
const mobile = wantsMobileUi();
// On a phone the naming model is slow, so names come from key columns until the user switches it on.
if (mobile && !modelNamingChosen()) setModelNaming(false);

const root = ReactDOM.createRoot(document.getElementById('root')!);
const render = () =>
  root.render(
    <React.StrictMode>
      {/* Every animation follows the system's reduce-motion setting. */}
      <MotionConfig reducedMotion="user">
        {mobile ? (
          <Suspense fallback={null}>
            <MobileApp />
          </Suspense>
        ) : (
          <App />
        )}
        {/* Every `title` shows as the app's own tooltip, not the system's. */}
        <TitleTooltips />
      </MotionConfig>
    </React.StrictMode>
  );

// The Rust engine (WebAssembly build) serves the small synchronous calls the UI makes (diagram
// builders, relation signatures); full runs use the native engine in the desktop app.
initEngine().then(render, err => {
  console.error('Failed to load the DBFlow engine', err);
  render();
});
