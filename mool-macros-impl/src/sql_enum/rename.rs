//! Rename policies for SQL enum labels and names.

use heck::{ToKebabCase, ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameRule {
    Snake,
    Kebab,
    Lowercase,
    Uppercase,
    Pascal,
    Camel,
}

impl RenameRule {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "snake_case" => Some(Self::Snake),
            "kebab-case" => Some(Self::Kebab),
            "lowercase" => Some(Self::Lowercase),
            "UPPERCASE" => Some(Self::Uppercase),
            "PascalCase" => Some(Self::Pascal),
            "camelCase" => Some(Self::Camel),
            _ => None,
        }
    }

    pub fn apply(self, value: &str) -> String {
        match self {
            Self::Snake => value.to_snake_case(),
            Self::Kebab => value.to_kebab_case(),
            Self::Lowercase => value.to_ascii_lowercase(),
            Self::Uppercase => value.to_ascii_uppercase(),
            Self::Pascal => value.to_upper_camel_case(),
            Self::Camel => value.to_lower_camel_case(),
        }
    }
}

pub fn default_sql_name(value: &str) -> String {
    value.to_snake_case()
}
