# Bundled fonts

These are unmodified static TrueType fonts from their official repositories.
Their full SIL Open Font License 1.1 notices are kept beside the font files.

| File | Source | SHA-256 |
| --- | --- | --- |
| `Geist-Regular.ttf` | [Geist](https://github.com/vercel/geist-font/blob/main/fonts/Geist/ttf/Geist-Regular.ttf) | `85a1c6b18a6b0a06dfe9fd4f6d6a5d4979f74ec861eaef4bc7868b5492b8a117` |
| `JetBrainsMono-Regular.ttf` | [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono/blob/master/fonts/ttf/JetBrainsMono-Regular.ttf) | `e6fd0d7e91550b3ed2b735d4312474362c4716edc4fc0577a0f61ed782d5aed1` |
| `JetBrainsMono-Bold.ttf` | [JetBrains Mono Bold](https://github.com/JetBrains/JetBrainsMono/blob/master/fonts/ttf/JetBrainsMono-Bold.ttf) | `d22c4f3821d725eb01210d278d95dfcfcaadc34699a06658d47c8a5cc5830ada` |

Geist is used for interface text; JetBrains Mono is used for terminal cells.
Geist uses regular weight; the terminal includes regular and bold. Shipping these files gives consistent text metrics
across supported operating systems without depending on installed fonts.

The app can load Nerd symbols, general symbols and CJK fallback fonts from
standard local font paths. These installed fonts are used at runtime and are
not redistributed with the app.
