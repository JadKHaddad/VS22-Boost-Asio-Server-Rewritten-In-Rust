use serde::{Deserialize, Serialize};
use rand::Rng;
pub struct Field {
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Eq, Hash, PartialEq, Deserialize, Serialize, Debug)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

impl Position {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let direction = rng.gen_range(0..4);
        match direction {
            0 => Self::Up,
            1 => Self::Down,
            2 => Self::Left,
            3 => Self::Right,
            _ => Self::Up,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub enum Message {
    Move(Direction),
    Position(Position),
    Score(i32),
    Disconnect,
}

impl Message {
    pub fn new_move(direction: Direction) -> Self {
        Self::Move(direction)
    }

    pub fn new_position(position: Position) -> Self {
        Self::Position(position)
    }

    pub fn new_score(score: i32) -> Self {
        Self::Score(score)
    }

    pub fn new_disconnect() -> Self {
        Self::Disconnect
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
