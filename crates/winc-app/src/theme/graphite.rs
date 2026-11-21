//! 「石墨」系列主题：Differ 默认的一深一浅两套配色。
//!
//! 浅色取自 mycim-stack 面板的基调：暖白的纸色底、近黑的暖灰字、藏青的强调色，细线和边都是
//! 压得很淡的暖灰。深色是中性的深灰：窗框和侧栏一个颜色，编辑区更深一档，蓝色的强调色。两套
//! 主题整块铺满的强调色上都是白字。语法色沿用 Winc 系列对比度的约束。

use super::*;

const fn hex(rgb: u32) -> Color {
    Color::hex(rgb)
}

/// 「石墨·昼」：默认的浅色主题，暖白的纸色卡片、近黑的暖灰字、藏青的主按钮。
pub static GRAPHITE_DAY: Palette = Palette {
    current_line: hex(0xF7F5EF),
    ..Palette::build(Tokens {
        dark: false,
        frame: hex(0xF3F1EA),
        card: hex(0xFCFBF8),
        popup: hex(0xFEFDFB),
        text: hex(0x363433),
        text_muted: hex(0x6B6A64),
        text_dim: hex(0xA39F92),
        accent: hex(0x144A74),
        accent_fill: hex(0x1C4B73),
        selection: hex(0xD9E4EF),
        gutter: [hex(0xB4B0A3), hex(0x6B6A64)],
        error: hex(0xA8322A),
        warning: hex(0x8A6410),
        git: [hex(0x2D618C), hex(0x3B7A55), hex(0xA03F2E), hex(0x7A4590)],
        tinted_tab: true,
        syntax: [
            hex(0x363433),
            hex(0x862617),
            hex(0x144A74),
            hex(0x2F6A47),
            hex(0x7A4E12),
            hex(0x74726A),
            hex(0x1F5F6B),
            hex(0x6A5A14),
            hex(0x6B3D82),
        ],
        ansi: [
            hex(0x363433),
            hex(0xB3392F),
            hex(0x2F7A4D),
            hex(0x8A6410),
            hex(0x2D618C),
            hex(0x7A4590),
            hex(0x1F6F7A),
            hex(0x77756C),
            hex(0x55534D),
            hex(0xC4493D),
            hex(0x3F8A5C),
            hex(0x9E7400),
            hex(0x3A75A8),
            hex(0x9052A8),
            hex(0x2A8794),
            hex(0xB9B6AA),
        ],
    })
};

/// 「石墨·夜」：默认的深色主题，中性的深灰窗框和侧栏、更深一档的编辑区、蓝色的强调色。
pub static GRAPHITE_NIGHT: Palette = Palette {
    sidebar_bg: hex(0x2B2D30),
    selected: hex(0x2E436E),
    ..Palette::build(Tokens {
        dark: true,
        frame: hex(0x2B2D30),
        card: hex(0x1E1F22),
        popup: hex(0x2B2D30),
        text: hex(0xDFE1E5),
        text_muted: hex(0x9DA0A8),
        text_dim: hex(0x6F737A),
        accent: hex(0x6A9BFA),
        accent_fill: hex(0x2F6BE4),
        selection: hex(0x214283),
        gutter: [hex(0x4B5059), hex(0xA1A3AB)],
        error: hex(0xF75464),
        warning: hex(0xF2C55C),
        git: [hex(0x6C9FE8), hex(0x73BD79), hex(0x868A91), hex(0xE06C75)],
        tinted_tab: true,
        syntax: [
            hex(0xBCBEC4),
            hex(0xCF8E6D),
            hex(0xB79CF2),
            hex(0x6AAB73),
            hex(0x2AACB8),
            hex(0x868A91),
            hex(0x56A8F5),
            hex(0xB3AE60),
            hex(0xD08BC4),
        ],
        ansi: [
            hex(0x2B2D30),
            hex(0xF75464),
            hex(0x5FB865),
            hex(0xD6AE58),
            hex(0x548AF7),
            hex(0xC77DBB),
            hex(0x2AACB8),
            hex(0xDFE1E5),
            hex(0x6F737A),
            hex(0xFF7A86),
            hex(0x73BD79),
            hex(0xF2C55C),
            hex(0x6BA0FF),
            hex(0xD896CB),
            hex(0x4CC3CE),
            hex(0xFFFFFF),
        ],
    })
};
