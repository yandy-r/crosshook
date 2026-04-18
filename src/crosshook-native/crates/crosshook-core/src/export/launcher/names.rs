const TRAINER_SUFFIX: &str = " - Trainer";

pub(crate) fn resolve_display_name(
    preferred_name: &str,
    steam_app_id: &str,
    trainer_path: &str,
) -> String {
    if !preferred_name.trim().is_empty() {
        return strip_trainer_suffix(preferred_name);
    }

    let trainer_name = std::path::Path::new(trainer_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .trim();

    if !trainer_name.is_empty() {
        return strip_trainer_suffix(trainer_name);
    }

    if !steam_app_id.trim().is_empty() {
        format!("steam-{steam_app_id}-trainer")
    } else {
        "crosshook-trainer".to_string()
    }
}

pub(crate) fn strip_trainer_suffix(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_suffix(TRAINER_SUFFIX)
        .unwrap_or(trimmed)
        .trim_end()
        .to_string()
}

pub fn sanitize_launcher_slug(value: &str) -> String {
    if value.trim().is_empty() {
        return "crosshook-trainer".to_string();
    }

    let mut slug = String::with_capacity(value.len());
    let mut last_character_was_separator = false;

    for character in value.trim().chars().flat_map(char::to_lowercase) {
        if character.is_alphanumeric() {
            slug.push(character);
            last_character_was_separator = false;
            continue;
        }

        if last_character_was_separator {
            continue;
        }

        slug.push('-');
        last_character_was_separator = true;
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "crosshook-trainer".to_string()
    } else {
        slug
    }
}
