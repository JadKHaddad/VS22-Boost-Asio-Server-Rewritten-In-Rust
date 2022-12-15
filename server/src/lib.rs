use parking_lot::RwLock;
use rand::Rng;
use shared::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

#[macro_use]
extern crate crossterm;
use crossterm::{
    cursor,
    style::Color,
    style::{Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{stdout, Write};

#[derive(Clone, Debug)]
pub struct Client {
    id: u16,
    position: Arc<RwLock<Position>>,
    score: Arc<RwLock<i32>>,
    sender: Sender<String>,
    color: Color,
}

impl Client {
    pub fn new(id: u16, position: Position, sender: Sender<String>, color: Color) -> Self {
        Self {
            id,
            position: Arc::new(RwLock::new(position.clone())),
            score: Arc::new(RwLock::new(0)),
            sender,
            color,
        }
    }

    pub fn adjust_score(&mut self, score: i32) {
        *self.score.write() += score;
    }

    pub fn set_position(&mut self, position: Position) {
        *self.position.write() = position;
    }
}

pub struct Game {
    clients: RwLock<Vec<Client>>,
    running: RwLock<bool>,
    max_clients: u16,
    field: Field,
}

impl Game {
    pub fn new(width: u16, height: u16, max_clients: u16) -> Self {
        Self {
            clients: RwLock::new(Vec::new()),
            running: RwLock::new(false),
            max_clients,
            field: Field { width, height },
        }
    }

    pub fn add_client(&self, client: Client) {
        self.clients.write().push(client);
    }

    pub fn remove_client(&self, id: u16) {
        self.clients.write().retain(|c| c.id != id);
    }

    pub fn get_total_clients(&self) -> u16 {
        self.clients.read().len() as u16
    }

    pub fn get_max_clients(&self) -> u16 {
        self.max_clients
    }

    pub fn create_random_position(&self) -> Position {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0..self.field.width) as u16;
        let y = rng.gen_range(0..self.field.height) as u16;
        Position { x, y }
    }

    pub fn create_random_color(&self) -> Color {
        let mut rng = rand::thread_rng();
        let color = rng.gen_range(0..255) as u8;
        Color::AnsiValue(color)
    }

    pub fn on_new_message(&self, client: &mut Client, msg: String) {
        if let Ok(msg) = Message::from_json(&msg) {
            match msg {
                Message::Move(direction) => {
                    self.adjust_position(client, direction);
                }
                Message::Disconnect => {}
                _ => {}
            }
        }
    }

    pub fn adjust_position(&self, client: &mut Client, direction: Direction) {
        let mut position = client.position.read().clone();
        match direction {
            Direction::Up => {
                if position.y == 0 {
                    position.y = self.field.height - 1;
                } else {
                    position.y -= 1;
                }
            }
            Direction::Down => {
                position.y += 1;
                if position.y >= self.field.height - 1 {
                    position.y = 0;
                }
            }
            Direction::Left => {
                if position.x == 0 {
                    position.x = self.field.width - 1;
                } else {
                    position.x -= 1;
                }
            }
            Direction::Right => {
                position.x += 1;
                if position.x >= self.field.width - 1 {
                    position.x = 0;
                }
            }
        }
        client.set_position(position);
    }

    fn display_field_once(&self) {
        let mut stdout = stdout();
        queue!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0)).unwrap();
        for _ in 0..self.field.height {
            for _ in 0..self.field.width {
                queue!(stdout, Print("X ")).unwrap();
            }
            queue!(stdout, cursor::MoveToNextLine(1)).unwrap();
        }
        queue!(stdout, cursor::MoveToNextLine(1)).unwrap();
        stdout.flush().unwrap();
    }

    fn refresh_field(&self) {
        let mut stdout = stdout();
        let clients = self.clients.read();
        let mut positions: HashMap<(u16, u16), (u16, Color)> = HashMap::new();
        for client in clients.iter() {
            let position = client.position.read();
            positions.insert((position.x, position.y), (client.id, client.color));
        }
        for i in 0..self.field.height {
            for j in 0..self.field.width {
                queue!(stdout, cursor::MoveTo(i * 2, j)).unwrap();
                if let Some((id, color)) = positions.get(&(i, j)) {
                    queue!(
                        stdout,
                        SetForegroundColor(*color),
                        Print(id.to_string()),
                        ResetColor
                    )
                    .unwrap();
                    continue;
                }
                queue!(stdout, Print("X"),).unwrap();
            }
        }
        stdout.flush().unwrap();
    }

    pub async fn run(&self) {
        if self.running.read().clone() {
            return;
        }
        *self.running.write() = true;

        self.display_field_once();
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            {
                let mut map: HashMap<Position, Vec<&mut Client>> = HashMap::new();

                let mut clients = self.clients.write();

                for client in clients.iter_mut() {
                    let position = client.position.clone();
                    let entry = map.entry(position.read().clone()).or_insert(Vec::new());
                    entry.push(client);
                }

                for (_, clients) in map.iter_mut() {
                    if clients.len() > 1 {
                        for client in clients.iter_mut() {
                            client.adjust_score(-5);
                            let new_position = self.create_random_position();
                            client.set_position(new_position);
                        }
                        continue;
                    }
                    for client in clients.iter_mut() {
                        client.adjust_score(1);
                    }
                }
            }
            self.refresh_field();
            for client in self.clients.read().iter() {
                let pos_msg = Message::new_position(client.position.read().clone())
                    .to_json()
                    .unwrap();
                let score_msg = Message::new_score(client.score.read().clone())
                    .to_json()
                    .unwrap();
                let sender = client.sender.clone();
                tokio::spawn(async move {
                    let _ = sender.send(pos_msg).await;
                    let _ = sender.send(score_msg).await;
                });
            }
        }
    }
}
