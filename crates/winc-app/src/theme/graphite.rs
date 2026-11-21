//! 「石墨」系列主题：Differ 默认的一深一浅两套配色。
//!
//! 取自 mycim-stack 面板的基调：冷灰的底（`#F2F4F6`）、墨色的字（`#18202B`）和黄色的强调色
//! （`#F2B705`）。整块铺满的强调色上用墨色的字，白字在黄底上看不清；浅色主题里细线、图标和
//! 链接用压暗的琥珀色，黄色在近白的底上对比度不足。语法色沿用 Winc 系列，对比度的约束相同。

use super::*;

const fn hex(rgb: u32) -> Color {
    Color::hex(rgb)
}

/// 墨色：浅色主题的正文，也是两套主题里黄底上的字。
const INK: Color = hex(0x18202B);
/// 黄色：两套主题整块铺满的强调色，深色主题的细线和图标。
const YELLOW: Color = hex(0xF2B705);

/// 「石墨·昼」：默认的浅色主题，冷灰窗框、近白卡片、墨色的字、黄色主按钮。
pub static GRAPHITE_DAY: Palette = Palette {
    text_on_accent: INK,
    ..Palette::build(Tokens {
        dark: false,
        frame: hex(0xF2F4F6),
        card: hex(0xFBFBFC),
        popup: hex(0xFFFFFF),
        text: INK,
        text_muted: hex(0x566070),
        text_dim: hex(0x98A0AC),
        accent: hex(0x8F6400),
        accent_fill: YELLOW,
        selection: hex(0xFBE6A2),
        gutter: [hex(0xA4AAB5), hex(0x4F5766)],
        error: hex(0xC2373A),
        warning: hex(0x8F6400),
        git: [hex(0x2B66D2), hex(0x16805F), hex(0xC4432F), hex(0xA43F98)],
        tinted_tab: true,
        syntax: [
            INK,
            hex(0xA43E3A),
            hex(0x156686),
            hex(0x1D691A),
            hex(0x58309D),
            hex(0x6B717D),
            hex(0x0A59CA),
            hex(0x574504),
            hex(0x7E2166),
        ],
        ansi: [
            INK,
            hex(0xC23B3B),
            hex(0x2E7D32),
            hex(0x8A6400),
            hex(0x2563C9),
            hex(0x8E3FB0),
            hex(0x0F7480),
            hex(0x6B7280),
            hex(0x575E6A),
            hex(0xD14B4B),
            hex(0x3C8F3F),
            hex(0x9E7400),
            hex(0x3577E0),
            hex(0xA052C4),
            hex(0x1A8794),
            hex(0xB4B9C2),
        ],
    })
};

/// 「石墨·夜」：默认的深色主题，墨色的底、冷灰的字、黄色的强调色。
pub static GRAPHITE_NIGHT: Palette = Palette {
    text_on_accent: INK,
    ..Palette::build(Tokens {
        dark: true,
        frame: hex(0x11171F),
        card: INK,
        popup: hex(0x222C3A),
        text: hex(0xE4E8ED),
        text_muted: hex(0x98A2B0),
        text_dim: hex(0x667080),
        accent: YELLOW,
        accent_fill: YELLOW,
        selection: hex(0x4D4424),
        gutter: [hex(0x505A69), hex(0xAEB6C2)],
        error: hex(0xF07F7F),
        warning: hex(0xF2C94C),
        git: [hex(0x72A7EC), hex(0x5FBFA0), hex(0xEB7F6C), hex(0xD898E0)],
        tinted_tab: true,
        syntax: [
            hex(0xE4E8ED),
            hex(0xD7915C),
            hex(0x6DDAD6),
            hex(0x75B18B),
            hex(0xC28EC4),
            hex(0x7F8796),
            hex(0x6FB7F0),
            hex(0xE4CF7A),
            hex(0xF9AEB9),
        ],
        ansi: [
            hex(0x2A323E),
            hex(0xEC7A7A),
            hex(0x8EC48A),
            hex(0xDDBB6E),
            hex(0x6FA6EE),
            hex(0xC890D0),
            hex(0x5FC6C2),
            hex(0xC9CED6),
            hex(0x6B7280),
            hex(0xF59A98),
            hex(0xA6D6A0),
            hex(0xEBCD88),
            hex(0x91BCF5),
            hex(0xDAAAE0),
            hex(0x80DAD6),
            hex(0xEEF0F4),
        ],
    })
};
