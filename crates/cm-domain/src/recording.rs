use std::path::PathBuf;

/// Papel de uma faixa de áudio gravada pelo OBS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackRole {
    Mix,
    Game,
    Comms,
    Mic,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack {
    pub index: u32,
    pub title: Option<String>,
    pub role: TrackRole,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub path: PathBuf,
    pub duration_s: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub audio_tracks: Vec<AudioTrack>,
}

fn fold(title: &str) -> String {
    title
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn role_from_title(title: &str) -> Option<TrackRole> {
    let t = fold(title);
    if t == "mix" {
        Some(TrackRole::Mix)
    } else if t.starts_with("jogo") {
        Some(TrackRole::Game)
    } else if t.starts_with("comunic") {
        Some(TrackRole::Comms)
    } else if t.starts_with("mic") {
        Some(TrackRole::Mic)
    } else {
        None
    }
}

/// Atribui papéis às faixas: títulos conhecidos mandam; sem nenhum título
/// conhecido, vale a posição (1 faixa = Mix; 4 faixas = Mix, Game, Comms, Mic).
pub fn assign_roles(titles: &[Option<String>]) -> Vec<TrackRole> {
    let known: Vec<Option<TrackRole>> = titles
        .iter()
        .map(|t| t.as_deref().and_then(role_from_title))
        .collect();
    if known.iter().any(Option::is_some) {
        return known
            .into_iter()
            .map(|r| r.unwrap_or(TrackRole::Unknown))
            .collect();
    }
    match titles.len() {
        1 => vec![TrackRole::Mix],
        4 => vec![
            TrackRole::Mix,
            TrackRole::Game,
            TrackRole::Comms,
            TrackRole::Mic,
        ],
        n => vec![TrackRole::Unknown; n],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(items: &[&str]) -> Vec<Option<String>> {
        items.iter().map(|s| Some((*s).to_string())).collect()
    }

    #[test]
    fn known_titles_map_to_roles() {
        let t = titles(&["Mix", "Jogo (padrao)", "Comunicacao", "Mic"]);
        assert_eq!(
            assign_roles(&t),
            vec![
                TrackRole::Mix,
                TrackRole::Game,
                TrackRole::Comms,
                TrackRole::Mic
            ]
        );
    }

    #[test]
    fn known_titles_ignore_accents_and_case() {
        let t = titles(&["MIX", "jogo", "Comunicação", "mic"]);
        assert_eq!(
            assign_roles(&t),
            vec![
                TrackRole::Mix,
                TrackRole::Game,
                TrackRole::Comms,
                TrackRole::Mic
            ]
        );
    }

    #[test]
    fn single_generic_track_is_mix() {
        assert_eq!(assign_roles(&titles(&["Track1"])), vec![TrackRole::Mix]);
    }

    #[test]
    fn untitled_single_track_is_mix() {
        assert_eq!(assign_roles(&[None]), vec![TrackRole::Mix]);
    }

    #[test]
    fn four_generic_tracks_are_positional() {
        let t = titles(&["Track1", "Track2", "Track3", "Track4"]);
        assert_eq!(
            assign_roles(&t),
            vec![
                TrackRole::Mix,
                TrackRole::Game,
                TrackRole::Comms,
                TrackRole::Mic
            ]
        );
    }

    #[test]
    fn no_tracks_gives_no_roles() {
        assert_eq!(assign_roles(&[]), Vec::<TrackRole>::new());
    }

    #[test]
    fn two_unknown_tracks_are_unknown() {
        let t = titles(&["Track1", "Track2"]);
        assert_eq!(
            assign_roles(&t),
            vec![TrackRole::Unknown, TrackRole::Unknown]
        );
    }
}
