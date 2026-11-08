# Third-Party Notices — nirs4all-ui

`nirs4all-ui` is distributed under `CeCILL-2.1 OR AGPL-3.0-or-later` (plus an optional
commercial license; see [`LICENSING.md`](LICENSING.md)). The package's ordinary entry points do
**not** vendor the components below — they are pulled from their official distributions — but
their licenses are acknowledged here as a courtesy and for compliance. Licenses are reported on
a best-effort basis; the authoritative text always ships with each upstream project.

**Exception — optional browser entry.** The self-contained browser bundle
(`dist/browser/nirs4all-ui-browser.mjs` and `dist/browser/nirs4all-ui-browser.iife.js`, exported
as `nirs4all-ui/browser` and `nirs4all-ui/browser/standalone`) **does** include compiled copies
of the following MIT-licensed packages:

| Bundled component | Version | License (SPDX) | Copyright | Full text |
|---|---|---|---|---|
| React (`react`) | 19.2.7 | MIT | Copyright (c) Meta Platforms, Inc. and affiliates. | [`LICENSES/THIRD-PARTY/react.LICENSE.txt`](LICENSES/THIRD-PARTY/react.LICENSE.txt) |
| React DOM (`react-dom`, client) | 19.2.7 | MIT | Copyright (c) Meta Platforms, Inc. and affiliates. | [`LICENSES/THIRD-PARTY/react-dom.LICENSE.txt`](LICENSES/THIRD-PARTY/react-dom.LICENSE.txt) |
| Scheduler (`scheduler`) | 0.27.0 | MIT | Copyright (c) Meta Platforms, Inc. and affiliates. | [`LICENSES/THIRD-PARTY/scheduler.LICENSE.txt`](LICENSES/THIRD-PARTY/scheduler.LICENSE.txt) |

Anyone redistributing that bundle (for example a host staging it into `vendor/ui/`) must keep
this notice and those license texts with it.

It is a shared React component and view-model package built on the npm/Node ecosystem.
Its direct dependencies are overwhelmingly **MIT**-licensed, with **Apache-2.0** used by
the TypeScript toolchain. Principal dependencies:

| Component | License (SPDX) | Upstream |
|---|---|---|
| React, React DOM | MIT | https://github.com/facebook/react |
| Vite, Vitest | MIT | https://github.com/vitejs/vite |
| `@types/react`, `@types/react-dom` | MIT | https://github.com/DefinitelyTyped/DefinitelyTyped |
| TypeScript | Apache-2.0 | https://github.com/microsoft/TypeScript |

For the exhaustive, version-pinned dependency tree and its licenses, run:

```text
npx license-checker --summary
```

License-family texts are bundled under [`LICENSES/`](LICENSES/): MIT, Apache-2.0. The exact upstream
texts of the components bundled by the browser entry are in
[`LICENSES/THIRD-PARTY/`](LICENSES/THIRD-PARTY/).
