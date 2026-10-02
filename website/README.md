# Neptune website

The landing page for [Neptune](https://neptune.rs): a static Next.js site styled with
Tailwind CSS. It uses [Bun](https://bun.sh) for dependencies and scripts.

```sh
bun install
bun run dev      # http://localhost:3000
bun run lint
bun run build    # static export in out/
```

`out/` holds plain files and can be served by any static host.

## Staying consistent with the app

The page rebuilds Neptune's window in the browser instead of showing screenshots,
so it follows the app's own sources:

- `src/app/globals.css` carries the palette, radii and motion of
  [`src/theme.rs`](../src/theme.rs) and [`docs/design.md`](../docs/design.md).
- `src/components/icons.tsx` is the icon set of [`src/icons.rs`](../src/icons.rs).
- `src/components/neptune/` mirrors the chrome, panes, palette and sheets under
  [`src/ui/`](../src/ui), and a small model of
  [`crates/neptune-model`](../crates/neptune-model).

When the app's look or behaviour changes, update the matching file here.

Website branding uses the supplied `Icon.png`, stored as
[`assets/branding/neptune-icon.png`](../assets/branding/neptune-icon.png).
Run `python3 scripts/export-logo.py` from the repository root to refresh the
navigation/footer logo, favicon and touch icon. The same logo is used in sharing
metadata. Native app icons use the separate platform logos documented in
[`assets/README.md`](../assets/README.md). Keep the window demo free of logos, as
in the desktop app.

The window answers a few typed commands through a stand-in shell
(`src/components/neptune/shell.ts`). It demonstrates the interface; it is not
Neptune's terminal engine.

## Claims

The page states only what the repository documents. Measurements come from
[`docs/performance.md`](../docs/performance.md) and are shown with their host and
scope; platform status follows the [README](../README.md). It makes no
comparison with other terminals. Keep new copy to the same standard.

## Hosting

The canonical domain is `https://neptune.rs`; metadata and sharing links use it.
Configure Vercel with `website/` as the project root and `main` as the production
branch.
[`vercel.json`](vercel.json) enables automatic deployments only for pushes to
`main`, including merged pull requests. Other branches do not create preview
deployments. The `**` rule covers branch names containing `/`.
Existing branches need to merge or rebase onto `main` after this configuration
lands so their commits include it.

The export assumes it is served from the root of a domain. To host it under a
subpath, such as a GitHub Pages project site, set `basePath` in
`next.config.ts`.
