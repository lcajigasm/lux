# Bundled fonts

Lux ships five developer fonts so they can be picked with
`editor.fontFamily` without installing anything. The decision, size budget,
and loading rules are in `docs/adr/0020-bundled-developer-fonts.md`.

Every font here is licensed under the SIL Open Font License, Version 1.1.
Each family's directory carries its license text (`OFL.txt`) with that
family's copyright notice. The files are unmodified copies of the upstream
release; nothing was subset, renamed, or re-hinted, so the Reserved Font
Name clauses do not apply. The fonts may be used, studied, copied, and
redistributed with Lux under those terms. They are not sold separately.

`SHA256SUMS` holds the SHA-256 of each shipped file.
`scripts/install-fonts.sh` checks it before copying this directory into a
package.

## JetBrains Mono 2.304

- Directory: `JetBrainsMono/` (`JetBrainsMono-Regular.ttf`, `OFL.txt`, `AUTHORS.txt`)
- Copyright 2020 The JetBrains Mono Project Authors
- Source: <https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304>
- Asset: `JetBrainsMono-2.304.zip`, SHA-256
  `6f6376c6ed2960ea8a963cd7387ec9d76e3f629125bc33d1fdcd7eb7012f7bbf`
- Programming ligatures through `calt`.

## Fira Code 6.2

- Directory: `FiraCode/` (`FiraCode-Regular.ttf`, `OFL.txt`)
- Copyright (c) 2014, The Fira Code Project Authors
- Source: <https://github.com/tonsky/FiraCode/releases/tag/6.2>
- Asset: `Fira_Code_v6.2.zip`, SHA-256
  `0949915ba8eb24d89fd93d10a7ff623f42830d7c5ffc3ecbf960e4ecad3e3e79`.
  The archive has no license file; `OFL.txt` is `LICENSE` from the
  repository at tag `6.2`.
- Programming ligatures through `calt`.

## IBM Plex Mono 2.5.0

- Directory: `IBMPlexMono/` (`IBMPlexMono-Regular.ttf`, `OFL.txt`)
- Copyright © 2017 IBM Corp., with Reserved Font Name "Plex"
- Source: <https://github.com/IBM/plex/releases/tag/%40ibm%2Fplex-mono%402.5.0>
- Asset: `ibm-plex-mono.zip`, SHA-256
  `6d23f01257663d8cc49a0d64c22ced630b79e0e2a0ac08a0da86e9a38bbc481c`
  (matches the digest GitHub publishes for the asset).
- No programming ligatures.

## Source Code Pro 2.042

- Directory: `SourceCodePro/` (`SourceCodePro-Regular.ttf`, `OFL.txt`)
- © 2023 Adobe, with Reserved Font Name 'Source'. Source is a trademark of
  Adobe in the United States and/or other countries.
- Source: <https://github.com/adobe-fonts/source-code-pro/releases/tag/2.042R-u%2F1.062R-i%2F1.026R-vf>
- Asset: `TTF-source-code-pro-2.042R-u_1.062R-i.zip`, SHA-256
  `0c85bac90d15c040b82939aa92bc8404420fccc02e37bbcb9c93a7f21abb52c6`.
  The archive has no license file; `OFL.txt` is `LICENSE.md` from the
  repository at the release tag.
- No programming ligatures.

## Cascadia Code 2407.24

- Directory: `CascadiaCode/` (`CascadiaCode-Regular.ttf`, `OFL.txt`)
- Copyright (c) 2019 - Present, Microsoft Corporation, with Reserved Font
  Name Cascadia Code.
- Source: <https://github.com/microsoft/cascadia-code/releases/tag/v2407.24>
- Asset: `CascadiaCode-2407.24.zip`, SHA-256
  `e67a68ee3386db63f48b9054bd196ea752bc6a4ebb4df35adce6733da50c8474`.
  The archive has no license file; `OFL.txt` is `LICENSE` from the
  repository at tag `v2407.24`.
- Programming ligatures through `calt`.
