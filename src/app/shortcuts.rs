use super::*;

pub(crate) fn is_copy_shortcut(key: &Key, modifiers: keyboard::Modifiers) -> bool {
    matches!(key.as_ref(), Key::Character("c") | Key::Character("C"))
        && (modifiers.command() || (modifiers.control() && modifiers.shift()))
}

pub(crate) fn is_paste_shortcut(key: &Key, modifiers: keyboard::Modifiers) -> bool {
    matches!(key.as_ref(), Key::Character("v") | Key::Character("V"))
        && (modifiers.command() || (modifiers.control() && modifiers.shift()))
}

pub(crate) fn is_close_tab_shortcut(
    shortcuts: &ShortcutSettings,
    key: &Key,
    physical_key: Physical,
    modifiers: keyboard::Modifiers,
) -> bool {
    shortcut_matches(&shortcuts.close_tab, key, physical_key, modifiers)
}

pub(crate) fn tab_switch_shortcut_index(
    shortcuts: &ShortcutSettings,
    key: &Key,
    physical_key: Physical,
    modifiers: keyboard::Modifiers,
) -> Option<usize> {
    shortcuts
        .tab_switches
        .iter()
        .position(|shortcut| shortcut_matches(shortcut, key, physical_key, modifiers))
}

pub(crate) fn is_previous_tab_shortcut(
    shortcuts: &ShortcutSettings,
    key: &Key,
    physical_key: Physical,
    modifiers: keyboard::Modifiers,
) -> bool {
    shortcut_matches(&shortcuts.previous_tab, key, physical_key, modifiers)
}

pub(crate) fn is_next_tab_shortcut(
    shortcuts: &ShortcutSettings,
    key: &Key,
    physical_key: Physical,
    modifiers: keyboard::Modifiers,
) -> bool {
    shortcut_matches(&shortcuts.next_tab, key, physical_key, modifiers)
}

pub(crate) fn is_open_settings_shortcut(
    shortcuts: &ShortcutSettings,
    key: &Key,
    physical_key: Physical,
    modifiers: keyboard::Modifiers,
) -> bool {
    shortcut_matches(&shortcuts.open_settings, key, physical_key, modifiers)
}

pub(crate) fn is_minimize_window_shortcut(
    shortcuts: &ShortcutSettings,
    key: &Key,
    physical_key: Physical,
    modifiers: keyboard::Modifiers,
) -> bool {
    shortcut_matches(&shortcuts.minimize_window, key, physical_key, modifiers)
}

pub(crate) fn shortcut_matches(
    shortcut: &str,
    key: &Key,
    physical_key: Physical,
    modifiers: keyboard::Modifiers,
) -> bool {
    let Some(pattern) = ShortcutPattern::parse(shortcut) else {
        return false;
    };

    pattern.matches(key, physical_key, modifiers)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutKey {
    Latin(char),
    Digit(u8),
    Comma,
    BracketLeft,
    BracketRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ShortcutPattern {
    command: bool,
    shift: bool,
    alt: bool,
    control: bool,
    logo: bool,
    key: Option<ShortcutKey>,
}

impl ShortcutPattern {
    fn parse(shortcut: &str) -> Option<Self> {
        let mut pattern = Self::default();

        for part in shortcut.split('+') {
            let token = part.trim().to_ascii_lowercase();
            if token.is_empty() {
                continue;
            }

            match token.as_str() {
                "command" | "cmd" => pattern.command = true,
                "shift" => pattern.shift = true,
                "alt" | "option" => pattern.alt = true,
                "control" | "ctrl" => pattern.control = true,
                "super" | "meta" | "logo" => pattern.logo = true,
                "," | "comma" => pattern.key = Some(ShortcutKey::Comma),
                "[" | "bracketleft" => pattern.key = Some(ShortcutKey::BracketLeft),
                "]" | "bracketright" => pattern.key = Some(ShortcutKey::BracketRight),
                digit if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() => {
                    pattern.key =
                        Some(ShortcutKey::Digit(digit.as_bytes()[0].saturating_sub(b'0')));
                }
                latin if latin.len() == 1 && latin.as_bytes()[0].is_ascii_alphabetic() => {
                    pattern.key =
                        Some(ShortcutKey::Latin(latin.chars().next().unwrap_or_default()));
                }
                _ => return None,
            }
        }

        pattern.key.map(|_| pattern)
    }

    fn matches(self, key: &Key, physical_key: Physical, modifiers: keyboard::Modifiers) -> bool {
        if modifiers.command() != self.command
            || modifiers.shift() != self.shift
            || modifiers.alt() != self.alt
        {
            return false;
        }

        #[cfg(target_os = "macos")]
        {
            if modifiers.control() != self.control {
                return false;
            }

            if self.logo {
                if !modifiers.logo() {
                    return false;
                }
            } else if !self.command && modifiers.logo() {
                return false;
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            if modifiers.logo() != self.logo {
                return false;
            }

            if self.control {
                if !modifiers.control() {
                    return false;
                }
            } else if !self.command && modifiers.control() {
                return false;
            }
        }

        match self.key {
            Some(ShortcutKey::Latin(expected)) => key
                .to_latin(physical_key)
                .map(|value| value.eq_ignore_ascii_case(&expected))
                .unwrap_or(false),
            Some(ShortcutKey::Digit(expected)) => key
                .to_latin(physical_key)
                .and_then(|value| value.to_digit(10))
                .map(|value| value as u8 == expected)
                .unwrap_or(false),
            Some(ShortcutKey::Comma) => match physical_key {
                Physical::Code(Code::Comma) => true,
                _ => matches!(key.as_ref(), Key::Character(",") | Key::Character("<")),
            },
            Some(ShortcutKey::BracketLeft) => match physical_key {
                Physical::Code(Code::BracketLeft) => true,
                _ => matches!(key.as_ref(), Key::Character("[") | Key::Character("{")),
            },
            Some(ShortcutKey::BracketRight) => match physical_key {
                Physical::Code(Code::BracketRight) => true,
                _ => matches!(key.as_ref(), Key::Character("]") | Key::Character("}")),
            },
            None => false,
        }
    }
}
