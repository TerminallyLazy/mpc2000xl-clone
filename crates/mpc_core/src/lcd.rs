use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdFrame {
    pub title: String,
    pub lines: [String; 4],
    pub soft_keys: [String; 6],
}

impl LcdFrame {
    pub fn main_screen(sequence_name: &str, tempo_bpm_x100: u32, playing: bool) -> Self {
        let tempo = format!("{}.{:02}", tempo_bpm_x100 / 100, tempo_bpm_x100 % 100);
        let status = if playing { "PLAY" } else { "STOP" };

        Self {
            title: "MAIN".to_string(),
            lines: [
                format!("Seq: {sequence_name}"),
                "Tr:01  Pgm:InitProgram".to_string(),
                format!("Tempo:{tempo}  {status}"),
                "Bars:001-001".to_string(),
            ],
            soft_keys: [
                "TrList".to_string(),
                "Track+".to_string(),
                "Track-".to_string(),
                "Solo".to_string(),
                "Erase".to_string(),
                "Edit".to_string(),
            ],
        }
    }

    pub fn mode_screen(title: &str, body: &str) -> Self {
        Self {
            title: title.to_string(),
            lines: [
                body.to_string(),
                "Source: core foundation".to_string(),
                "Evidence: unmapped".to_string(),
                "Ready for fixtures".to_string(),
            ],
            soft_keys: [
                "F1".to_string(),
                "F2".to_string(),
                "F3".to_string(),
                "F4".to_string(),
                "F5".to_string(),
                "F6".to_string(),
            ],
        }
    }
}
