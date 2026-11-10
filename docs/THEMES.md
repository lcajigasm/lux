# Bundled color themes

Monokai stays the default (`workbench.colorTheme` is `monokai`). The other
files in `themes/` are the same `.sublime-color-scheme` format, parsed by
`color_scheme::parse_scheme`. Each file uses Lux's short scope names. The
hex values are the published palette of the project named below, not a
copy of that project's theme JSON.

A drop-in file in the user config `themes/` directory is loaded the same
way and overrides a bundled name.

The theme picker, the Settings color-theme menu, and the palette rows
`Theme: …` preview the highlighted theme. Escape restores the theme that
was active when that list opened and does not write `settings.json`.
Enter confirms it and writes `workbench.colorTheme`.

## Sources

| Theme | File | Source | Copyright | License |
| --- | --- | --- | --- | --- |
| Monokai | `themes/Monokai.sublime-color-scheme` | [VS Code theme-monokai](https://github.com/microsoft/vscode/tree/main/extensions/theme-monokai) | Microsoft Corporation | MIT. See `themes/NOTICE.md`. |
| One Dark | `themes/One Dark.sublime-color-scheme` | [atom/one-dark-syntax](https://github.com/atom/one-dark-syntax) `styles/colors.less` | Copyright (c) 2014 GitHub Inc. | MIT |
| Dracula | `themes/Dracula.sublime-color-scheme` | [dracula/dracula-theme](https://github.com/dracula/dracula-theme) | Copyright (c) 2018 Dracula Theme | MIT |
| Solarized Dark, Solarized Light | `themes/Solarized Dark.sublime-color-scheme`, `themes/Solarized Light.sublime-color-scheme` | [altercation/solarized](https://github.com/altercation/solarized) | Copyright (c) 2011 Ethan Schoonover | MIT |
| Nord | `themes/Nord.sublime-color-scheme` | [nordtheme/nord](https://github.com/nordtheme/nord) | Copyright (c) Sven Greb | MIT |
| Gruvbox | `themes/Gruvbox.sublime-color-scheme` | [morhetz/gruvbox](https://github.com/morhetz/gruvbox) | Copyright (c) 2014 Pavel Pertsev | MIT |
| Tokyo Night | `themes/Tokyo Night.sublime-color-scheme` | [enkia/tokyo-night-vscode](https://github.com/enkia/tokyo-night-vscode) | Copyright (c) enkia | MIT |
| Catppuccin, Catppuccin Latte | `themes/Catppuccin.sublime-color-scheme`, `themes/Catppuccin Latte.sublime-color-scheme` | [catppuccin/catppuccin](https://github.com/catppuccin/catppuccin) Mocha and Latte | Copyright (c) 2021 Catppuccin | MIT |
| GitHub Dark, GitHub Light | `themes/GitHub Dark.sublime-color-scheme`, `themes/GitHub Light.sublime-color-scheme` | [primer/github-vscode-theme](https://github.com/primer/github-vscode-theme) | Copyright (c) GitHub, Inc. | MIT |
| Ayu | `themes/Ayu.sublime-color-scheme` | [ayu-theme/ayu-colors](https://github.com/ayu-theme/ayu-colors) dark | Copyright (c) Dempfi | MIT |
| Rose Pine | `themes/Rose Pine.sublime-color-scheme` | [rose-pine/rose-pine](https://github.com/rose-pine/rose-pine) | Copyright (c) 2021 Rosé Pine | MIT |

Catppuccin Latte is the light palette of the same MIT project as the
Catppuccin (Mocha) file. Rose Pine is included because it is a widely used
editor theme and its repository is MIT.

Monokai Pro is proprietary and is not included. The 2006 `Monokai.tmTheme`
has no license that allows redistribution, so it is not vendored.
