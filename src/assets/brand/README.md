# OurFault brand assets

| File | Use |
| --- | --- |
| `ourfault-logo.webp` | Horizontal logo (transparent). Welcome screen. Designed for dark backgrounds: the "Our" wordmark is ice-white. |
| `ourfault-emblem.webp` | The OF emblem (transparent), cut from the horizontal logo. Large artwork on the welcome screen. |
| `ourfault-icon.png` | Square application icon (rounded, transparent corners). Top bar, loading state. |

The Windows icons in `src-tauri/icons/` and `public/favicon.png` are generated
from the same application icon (`npx tauri icon <1024px png>`).

All files derive from the OurFault reference artwork supplied by the product
owner. To replace them (e.g. with vector originals), keep the file names, or
update the imports in `src/App.tsx` and `src/features/entry/EntryScreen.tsx`.
Do not redraw or approximate the logo in code.
