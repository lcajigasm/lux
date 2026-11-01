# Monokai theme notice

`Monokai.sublime-color-scheme` is Lux's default dark theme. It is an
adaptation of the Monokai palette published by Microsoft in the MIT-licensed
Visual Studio Code extension `extensions/theme-monokai`:

- Source: <https://github.com/microsoft/vscode/tree/main/extensions/theme-monokai>
- File consulted: `themes/monokai-color-theme.json`
- Copyright (c) Microsoft Corporation
- License: MIT (see below)

The VS Code file's header documents the original Monokai surfaces this
scheme uses: `#1E1F1C` (tab well and sidebar), `#272822` (editor
background), `#414339` (status bar and input), `#75715E` (muted / focus),
and `#F8F8F2` (foreground). Line highlight `#3E3D32` and selection
`#49483E` are the classic Sublime Monokai values. Token colors are the
classic set (comment `#75715E`, keyword `#F92672`, string `#E6DB74`,
number `#AE81FF`, type `#66D9EF`, function `#A6E22E`, parameter
`#FD971F`). The current VS Code JSON shifts the comment color to
`#88846F`; this file keeps `#75715E`.

Rules use Lux's short scope names (`comment`, `keyword`, `string`, …) so
the highlighter hits them directly. A `.sublime-color-scheme` dropped in
the user config `themes/` directory is loaded the same way. TextMate
scopes such as `entity.name.function` are mapped onto those short names.

Monokai Pro is proprietary and is not included. The 2006 `Monokai.tmTheme`
has no license that allows redistribution, so it is not vendored.

## MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
