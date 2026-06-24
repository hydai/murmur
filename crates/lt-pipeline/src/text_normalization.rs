use std::sync::OnceLock;

use ferrous_opencc::{config::BuiltinConfig, OpenCC};

static OPENCC_S2TWP: OnceLock<Result<OpenCC, String>> = OnceLock::new();

const TAIWAN_TERM_OVERRIDES: &[(&str, &str)] = &[("自定義", "自訂")];

const FALLBACK_TAIWAN_REPLACEMENTS: &[(&str, &str)] = &[
    ("简体中文", "簡體中文"),
    ("繁体中文", "繁體中文"),
    ("兼容", "相容"),
    ("自定义", "自訂"),
    ("自定義", "自訂"),
    ("端口", "埠"),
    ("设定", "設定"),
    ("设置", "設定"),
    ("软件", "軟體"),
    ("数据库", "資料庫"),
    ("数据", "資料"),
    ("内存", "記憶體"),
    ("服务器", "伺服器"),
    ("复制", "複製"),
];

pub(crate) fn normalize_final_output(text: &str) -> String {
    if text.is_empty() || contains_japanese_kana(text) {
        return text.to_string();
    }

    let converted = match opencc_s2twp() {
        Ok(opencc) => opencc.convert(text),
        Err(error) => {
            tracing::error!(error, "OpenCC S2TWP initialization failed");
            fallback_normalize(text)
        }
    };

    apply_taiwan_term_overrides(&converted)
}

fn opencc_s2twp() -> Result<&'static OpenCC, &'static str> {
    OPENCC_S2TWP
        .get_or_init(|| {
            OpenCC::from_config(BuiltinConfig::S2twp).map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(String::as_str)
}

fn apply_taiwan_term_overrides(text: &str) -> String {
    replace_terms(text, TAIWAN_TERM_OVERRIDES)
}

fn fallback_normalize(text: &str) -> String {
    replace_terms(text, FALLBACK_TAIWAN_REPLACEMENTS)
        .chars()
        .map(simplified_char_to_traditional)
        .collect()
}

fn replace_terms(text: &str, terms: &[(&str, &str)]) -> String {
    let mut normalized = text.to_string();
    for (from, to) in terms {
        normalized = normalized.replace(from, to);
    }
    normalized
}

fn contains_japanese_kana(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c, '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'))
}

fn simplified_char_to_traditional(c: char) -> char {
    match c {
        '个' => '個',
        '们' => '們',
        '体' => '體',
        '会' => '會',
        '刚' => '剛',
        '发' => '發',
        '帮' => '幫',
        '并' => '並',
        '对' => '對',
        '尝' => '嘗',
        '检' => '檢',
        '现' => '現',
        '简' => '簡',
        '络' => '絡',
        '义' => '義',
        '设' => '設',
        '试' => '試',
        '遗' => '遺',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_simplified_chinese_to_traditional_taiwan_usage() {
        let input = "我刚才尝试对 STT 提供一个新的 OpenAI 兼容的自定义端口，但我发现它并不能成功地被设定加入。帮我检查我们目前自定义的功能是否有遗漏。";

        let output = normalize_final_output(input);

        assert_eq!(
            output,
            "我剛才嘗試對 STT 提供一個新的 OpenAI 相容的自訂埠，但我發現它並不能成功地被設定加入。幫我檢查我們目前自訂的功能是否有遺漏。"
        );
    }

    #[test]
    fn uses_opencc_taiwan_phrase_conversion() {
        let input = "鼠标里面的硅二极管坏了，导致光标分辨率降低。";

        let output = normalize_final_output(input);

        assert_eq!(output, "滑鼠裡面的矽二極體壞了，導致游標解析度降低。");
    }

    #[test]
    fn normalizes_common_technical_terms_to_taiwan_usage() {
        let input = "软件会把数据库数据复制到服务器内存。";

        let output = normalize_final_output(input);

        assert_eq!(output, "軟體會把資料庫資料複製到伺服器記憶體。");
    }

    #[test]
    fn skips_han_conversion_when_text_contains_japanese_kana() {
        let input = "日本語の国際会議について話します。";

        let output = normalize_final_output(input);

        assert_eq!(output, input);
    }
}
