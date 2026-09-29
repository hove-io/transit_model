// Copyright (C) 2017 Hove and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! Filename slug generation for `line_*.xml`, see NeTEx-fr profile.

use unicode_normalization::UnicodeNormalization;

/// Replace known ligatures and remove diacritical marks from Latin letters.
///
/// Ligatures (`œ`, `æ`, `ß`, `ø`) aren't split by Unicode normalization, so
/// they're replaced by hand first (e.g. "œ" -> "oe").
///
/// Remaining accented letters are then split via NFKD decomposition (e.g.
/// "é" becomes 'e' + U+0301), and the combining-mark characters are filtered
/// out, keeping only the base letters (e.g. "é" -> "e", "à" -> "a", "ü" -> "u").
fn strip_accents(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'œ' | 'Œ' => result.push_str("oe"),
            'æ' | 'Æ' => result.push_str("ae"),
            'ß' => result.push_str("ss"),
            'ø' | 'Ø' => result.push('o'),
            _ => result.push(c),
        }
    }
    result
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

/// Replace any run of characters that are neither `a`-`z` nor `0`-`9` with a
/// single `_`, trimming any leading/trailing `_`.
/// e.g. "Nord - Pas-de-Calais!" -> "Nord_Pas_de_Calais".
fn collapse_non_alphanumeric(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_was_underscore = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            result.push(c);
            last_was_underscore = false;
        } else if !last_was_underscore && !result.is_empty() {
            result.push('_');
            last_was_underscore = true;
        }
    }
    if result.ends_with('_') {
        result.pop();
    }
    result
}

/// Truncate to `max_len`, keeping whole `_`-separated segments.
///
/// Example (`max_len = 24`, the cut falls right after a `_`):
/// "croix_rousse_plateau_de_saint_rambert" -> "croix_rousse_plateau_de"
///
/// Example (`max_len = 22`, the cut falls mid-word, in "de"): the incomplete
/// trailing fragment ("_d") is dropped entirely, not just its last character:
/// "croix_rousse_plateau_de_saint_rambert" -> "croix_rousse_plateau"
///
/// If the first `max_len` characters contain no `_` at all, the text is cut
/// exactly at `max_len`, even if that splits a word in the middle.
fn truncate_at_segment_boundary(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_len).collect();
    match truncated.rfind('_') {
        Some(pos) => truncated[..pos].to_string(),
        None => truncated,
    }
}

/// Clean up a string for use in a filename.
///
/// See ["Export sous forme de fichier"](https://github.com/etalab/transport-profil-netex-fr/blob/ab7ac9038c30bdf6b28cb5bfa3424ac6e172b912/NeTEx/elements_communs/index.md#export-sous-forme-de-fichier)
/// in the NeTEx-fr profile: no uppercase, `_` separator, no accent, no space,
/// max 250 characters.
pub(in crate::netex_france) fn slug(text: &str, max_len: usize) -> String {
    let text = strip_accents(text);
    let text = text.to_lowercase();
    let text = collapse_non_alphanumeric(&text);
    truncate_at_segment_boundary(&text, max_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod strip_accents {
        use super::*;

        #[test]
        fn replaces_ligatures() {
            assert_eq!(strip_accents("cœur"), "coeur");
            assert_eq!(strip_accents("Straße"), "Strasse");
            assert_eq!(strip_accents("øre"), "ore");
        }

        #[test]
        fn removes_diacritics() {
            assert_eq!(strip_accents("Île"), "Ile");
            assert_eq!(strip_accents("à"), "a");
            assert_eq!(strip_accents("ü"), "u");
        }

        #[test]
        fn replaces_uppercase_ligatures() {
            assert_eq!(strip_accents("Œ"), "oe");
            assert_eq!(strip_accents("Æ"), "ae");
            assert_eq!(strip_accents("Ø"), "o");
        }

        #[test]
        fn empty_string_is_empty() {
            assert_eq!(strip_accents(""), "");
        }
    }

    mod collapse_non_alphanumeric {
        use super::*;

        #[test]
        fn merges_separator_runs() {
            assert_eq!(
                collapse_non_alphanumeric("Nord - Pas-de-Calais!"),
                "Nord_Pas_de_Calais"
            );
        }

        #[test]
        fn trims_leading_and_trailing_separators() {
            assert_eq!(collapse_non_alphanumeric("  hello  "), "hello");
        }

        #[test]
        fn empty_string_is_empty() {
            assert_eq!(collapse_non_alphanumeric(""), "");
        }

        #[test]
        fn only_separators_is_empty() {
            assert_eq!(collapse_non_alphanumeric("   - !! -- "), "");
        }

        #[test]
        fn keeps_digits() {
            assert_eq!(
                collapse_non_alphanumeric("Route 2024 Express"),
                "Route_2024_Express"
            );
        }

        #[test]
        fn already_clean_input_is_unchanged() {
            assert_eq!(collapse_non_alphanumeric("HelloWorld123"), "HelloWorld123");
        }
    }

    mod truncate_at_segment_boundary {
        use super::*;

        #[test]
        fn cuts_right_after_separator() {
            assert_eq!(
                truncate_at_segment_boundary("croix_rousse_plateau_de_saint_rambert", 24),
                "croix_rousse_plateau_de"
            );
        }

        #[test]
        fn drops_incomplete_trailing_word() {
            assert_eq!(
                truncate_at_segment_boundary("croix_rousse_plateau_de_saint_rambert", 22),
                "croix_rousse_plateau"
            );
        }

        #[test]
        fn keeps_short_text_untouched() {
            assert_eq!(truncate_at_segment_boundary("short", 24), "short");
        }

        #[test]
        fn empty_string_is_empty() {
            assert_eq!(truncate_at_segment_boundary("", 24), "");
        }

        #[test]
        fn max_len_zero() {
            assert_eq!(truncate_at_segment_boundary("hello", 0), "");
        }

        #[test]
        fn does_not_panic_on_multi_byte_char_at_boundary() {
            // "café" has 4 characters but 5 bytes ('é' is 2 bytes in UTF-8),
            // so max_len=4 falls inside that character if bytes are counted
            // instead of characters.
            assert_eq!(truncate_at_segment_boundary("café", 4), "café");
            assert_eq!(truncate_at_segment_boundary("café", 3), "caf");
        }

        #[test]
        fn keeps_text_of_exactly_max_len_untouched() {
            assert_eq!(truncate_at_segment_boundary("abcde", 5), "abcde");
        }

        #[test]
        fn cuts_raw_when_no_separator_found() {
            // No '_' in the first `max_len` characters: cut exactly at
            // max_len, even mid-word.
            assert_eq!(truncate_at_segment_boundary("helloworld", 5), "hello");
        }
    }

    mod slug {
        use super::*;

        #[test]
        fn full_pipeline() {
            assert_eq!(slug("Nord - Pays de la Loire", 30), "nord_pays_de_la_loire");
            assert_eq!(slug("Île-de-France", 30), "ile_de_france");
            assert_eq!(slug("Cœur de Loire", 30), "coeur_de_loire");
            assert_eq!(
                slug("croix_rousse_plateau_de_saint_rambert", 24),
                "croix_rousse_plateau_de"
            );
        }

        #[test]
        fn empty_string_is_empty() {
            assert_eq!(slug("", 30), "");
        }

        #[test]
        fn only_punctuation_is_empty() {
            assert_eq!(slug("!!! --- ???", 30), "");
        }

        #[test]
        fn max_len_zero_is_empty() {
            assert_eq!(slug("Hello", 0), "");
        }

        #[test]
        fn non_latin_script_falls_back_to_empty() {
            assert_eq!(slug("Москва", 30), "");
        }

        #[test]
        fn keeps_digits() {
            assert_eq!(slug("Ligne 12", 30), "ligne_12");
        }

        #[test]
        fn apostrophe_is_a_separator() {
            assert_eq!(slug("Gare d'Austerlitz", 30), "gare_d_austerlitz");
        }

        #[test]
        fn truncates_a_realistic_long_accented_name() {
            assert_eq!(
                slug("Réseau de Transport de l'Agglomération Grenobloise", 40),
                "reseau_de_transport_de_l_agglomeration"
            );
        }

        #[test]
        fn mixed_latin_and_non_latin_script() {
            assert_eq!(slug("Réseau Москва Est", 30), "reseau_est");
        }
    }
}
