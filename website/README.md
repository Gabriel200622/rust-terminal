# Pace website

The landing page for [Pace](../README.md): a static Next.js site styled with
Tailwind CSS. It uses [Bun](https://bun.sh) for dependencies and scripts.

```sh
bun install
bun run dev      # http://localhost:3000
bun run lint
bun run build    # static export in out/
```

`out/` holds plain files and can be served by any static host.

## Staying consistent with the app

The page rebuilds Pace's window in the browser instead of showing screenshots,
so it follows the app's own sources:

- `src/app/globals.css` carries the palette, radii and motion of
  [`src/theme.rs`](../src/theme.rs) and [`docs/design.md`](../docs/design.md).
- `src/components/icons.tsx` is the icon set of [`src/icons.rs`](../src/icons.rs).
- `src/components/pace/` mirrors the chrome, panes, palette and sheets under
  [`src/ui/`](../src/ui), and a small model of
  [`crates/pace-model`](../crates/pace-model).

When the app's look or behaviour changes, update the matching file here.

The window answers a few typed commands through a stand-in shell
(`src/components/pace/shell.ts`). It demonstrates the interface; it is not
Pace's terminal engine.

## Claims

The page states only what the repository documents. Measurements come from
[`docs/performance.md`](../docs/performance.md) and are shown with their host and
scope; platform status follows the [README](../README.md). It makes no
comparison with other terminals. Keep new copy to the same standard.

## Hosting

The export assumes it is served from the root of a domain. To host it under a
subpath, such as a GitHub Pages project site, set `basePath` in
`next.config.ts`.
