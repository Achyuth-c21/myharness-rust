Icons are intentionally absent from the scaffold. Generate them once you have a brand asset:

    cargo tauri icon path/to/source.png

That emits the full icon set expected by Tauri 2 bundling (platform icons + `icon.png`).
Keep this directory in git once populated; delete this README then.
