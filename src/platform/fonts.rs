//! Bundled fonts and bounded platform font discovery.

use eframe::egui::{self, FontFamily};
use std::{
    io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub fn install(ctx: &egui::Context) {
    // Fixed candidate paths avoid a recursive scan of every installed font.
    // Definitions share font bytes through Arc and are loaded once per process.
    static DEFINITIONS: OnceLock<egui::FontDefinitions> = OnceLock::new();
    ctx.set_fonts(DEFINITIONS.get_or_init(font_definitions).clone());
}

/// Uses only bundled bytes, giving tests the same metrics on every desktop.
pub fn bundled_definitions() -> egui::FontDefinitions {
    font_definitions_with(&mut |_| Err(io::Error::from(io::ErrorKind::NotFound)))
}

fn font_definitions() -> egui::FontDefinitions {
    font_definitions_with(&mut |path| std::fs::read(path))
}

fn font_definitions_with(
    read: &mut impl FnMut(&Path) -> io::Result<Vec<u8>>,
) -> egui::FontDefinitions {
    let mut definitions = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "Geist",
            include_bytes!("../../assets/fonts/Geist-Regular.ttf").as_slice(),
        ),
        (
            "JetBrains",
            include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf").as_slice(),
        ),
        (
            "JetBrainsBold",
            include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf").as_slice(),
        ),
    ] {
        definitions
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
    }

    let mut terminal_fallbacks = Vec::new();
    let mut shared_fallbacks = Vec::new();
    if let Some(name) = load_font(&mut definitions, "SystemNerd", nerd_candidates(), 0, read) {
        terminal_fallbacks.push(name);
    }
    for (name, candidates, index) in system_fallback_candidates() {
        if let Some(name) = load_font(&mut definitions, name, candidates, index, read) {
            terminal_fallbacks.push(name.clone());
            shared_fallbacks.push(name);
        }
    }

    let mono = definitions
        .families
        .entry(FontFamily::Monospace)
        .or_default();
    mono.insert(0, "JetBrains".into());
    // The bundled terminal font owns Latin metrics. Installed Nerd symbols and
    // CJK faces are fallback only, so prompts do not change the normal grid.
    mono.splice(1..1, terminal_fallbacks);
    let mut bold = mono.clone();
    bold.insert(0, "JetBrainsBold".into());
    definitions
        .families
        .insert(FontFamily::Name("TerminalBold".into()), bold);

    let proportional = definitions
        .families
        .entry(FontFamily::Proportional)
        .or_default();
    proportional.insert(0, "Geist".into());
    proportional.extend(shared_fallbacks);
    definitions
}

/// Load installed fonts only. Patched Nerd fonts are never redistributed.
fn load_font(
    definitions: &mut egui::FontDefinitions,
    name: &str,
    candidates: Vec<PathBuf>,
    preferred_index: u32,
    read: &mut impl FnMut(&Path) -> io::Result<Vec<u8>>,
) -> Option<String> {
    for path in candidates {
        let Ok(bytes) = read(&path) else {
            continue;
        };
        let Some(signature) = bytes.get(..4) else {
            continue;
        };
        let index = match signature {
            b"ttcf" => {
                let Some(count) = bytes
                    .get(8..12)
                    .and_then(|data| data.try_into().ok())
                    .map(u32::from_be_bytes)
                else {
                    continue;
                };
                if count == 0 {
                    continue;
                }
                if preferred_index < count {
                    preferred_index
                } else {
                    0
                }
            }
            b"\x00\x01\x00\x00" | b"OTTO" | b"true" => 0,
            _ => continue,
        };
        let mut data = egui::FontData::from_owned(bytes);
        data.index = index;
        definitions.font_data.insert(name.into(), data.into());
        return Some(name.into());
    }
    None
}

fn nerd_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Some(user_font_home) = std::env::var_os("HOME") {
        let user_font_home = PathBuf::from(user_font_home);
        #[cfg(target_os = "linux")]
        roots.extend([
            user_font_home.join(".local/share/fonts"),
            user_font_home.join(".fonts"),
        ]);
        #[cfg(target_os = "macos")]
        roots.push(user_font_home.join("Library/Fonts"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local_data) = std::env::var_os("LOCALAPPDATA") {
            roots.push(PathBuf::from(local_data).join("Microsoft/Windows/Fonts"));
        }
        roots.push(windows_fonts());
    }
    #[cfg(target_os = "linux")]
    roots.extend([
        PathBuf::from("/usr/local/share/fonts"),
        PathBuf::from("/usr/share/fonts/truetype"),
    ]);
    #[cfg(target_os = "macos")]
    roots.push(PathBuf::from("/Library/Fonts"));

    let names = [
        "MesloLGSNerdFontMono-Regular.ttf",
        "MesloLGMNerdFontMono-Regular.ttf",
        "MesloLGS NF Regular.ttf",
        "JetBrainsMonoNerdFontMono-Regular.ttf",
        "SymbolsNerdFontMono-Regular.ttf",
    ];
    let mut candidates = Vec::new();
    for root in roots {
        for directory in ["", "Meslo", "JetBrainsMono", "NerdFonts", "SymbolsNerdFont"] {
            for name in names {
                candidates.push(root.join(directory).join(name));
            }
        }
    }
    candidates
}

type SystemFallback = (&'static str, Vec<PathBuf>, u32);

fn system_fallback_candidates() -> Vec<SystemFallback> {
    #[cfg(target_os = "linux")]
    {
        vec![
            (
                "SystemSymbols",
                paths(&[
                    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
                    "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
                    "/usr/share/fonts/google-noto-sans-symbols-2-fonts/NotoSansSymbols2-Regular.ttf",
                ]),
                0,
            ),
            (
                "SystemSymbolsLegacy",
                paths(&[
                    "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
                    "/usr/share/fonts/noto/NotoSansSymbols-Regular.ttf",
                ]),
                0,
            ),
            (
                "SystemCjk",
                paths(&[
                    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                    "/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc",
                ]),
                2,
            ),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            (
                "SystemSymbols",
                paths(&["/System/Library/Fonts/Apple Symbols.ttf"]),
                0,
            ),
            (
                "SystemCjk",
                paths(&[
                    "/System/Library/Fonts/PingFang.ttc",
                    "/System/Library/Fonts/STHeiti Light.ttc",
                    "/Library/Fonts/NotoSansCJK-Regular.ttc",
                ]),
                0,
            ),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        let root = windows_fonts();
        vec![
            ("SystemSymbols", vec![root.join("seguisym.ttf")], 0),
            (
                "SystemCjk",
                ["msyh.ttc", "Deng.ttf", "simhei.ttf", "msgothic.ttc"]
                    .map(|name| root.join(name))
                    .into(),
                0,
            ),
        ]
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    Vec::new()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn paths(candidates: &[&str]) -> Vec<PathBuf> {
    candidates.iter().map(PathBuf::from).collect()
}

#[cfg(target_os = "windows")]
fn windows_fonts() -> PathBuf {
    PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into())).join("Fonts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_fonts_do_not_depend_on_installed_fallbacks() {
        let definitions = bundled_definitions();
        assert_eq!(definitions.families[&FontFamily::Monospace][0], "JetBrains");
        assert_eq!(
            definitions.families[&FontFamily::Name("TerminalBold".into())][0],
            "JetBrainsBold"
        );
        assert!(!definitions.font_data.contains_key("SystemNerd"));
        assert!(!definitions.font_data.contains_key("SystemCjk"));
    }

    #[test]
    fn invalid_candidates_are_skipped_before_valid_font() {
        let mut definitions = egui::FontDefinitions::default();
        let mut read = |path: &Path| {
            Ok(if path == Path::new("broken") {
                vec![0; 16]
            } else {
                include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf").to_vec()
            })
        };
        assert_eq!(
            load_font(
                &mut definitions,
                "Candidate",
                vec!["broken".into(), "valid".into()],
                0,
                &mut read
            ),
            Some("Candidate".into())
        );
        assert_eq!(definitions.font_data["Candidate"].index, 0);
    }
}
