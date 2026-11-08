# Third-party licence texts bundled by the browser entry

The optional browser entry (`dist/browser/nirs4all-ui-browser.mjs` and
`dist/browser/nirs4all-ui-browser.iife.js`, built by `vite.browser.config.ts`) bundles
the third-party packages below. Their MIT licences require the copyright and
permission notice to accompany copies, so the exact upstream texts are kept here.

| Package | Bundled version | Licence | Text | Copied from |
| --- | --- | --- | --- | --- |
| `react` | 19.2.7 | MIT | [`react.LICENSE.txt`](react.LICENSE.txt) | `node_modules/react/LICENSE` |
| `react-dom` (client) | 19.2.7 | MIT | [`react-dom.LICENSE.txt`](react-dom.LICENSE.txt) | `node_modules/react-dom/LICENSE` |
| `scheduler` | 0.27.0 | MIT | [`scheduler.LICENSE.txt`](scheduler.LICENSE.txt) | `node_modules/scheduler/LICENSE` |

All three texts are byte-identical upstream (SHA-256
`da6d3703ed11cbe42bd212c725957c98da23cbff1998c05fa4b3d976d1a58e93`,
"Copyright (c) Meta Platforms, Inc. and affiliates."). Versions are the ones resolved
in this checkout's `node_modules`; if a dependency update changes them, refresh the
table and the texts together.

Hosts that stage the browser artifacts (for example into a `vendor/ui/` directory)
copy `THIRD_PARTY_NOTICES.md` and this directory's licence texts alongside
`nirs4all-ui-browser.iife.js` and `browser.css`.
