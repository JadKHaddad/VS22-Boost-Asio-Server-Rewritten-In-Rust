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
    event::{read, Event, KeyCode, KeyEvent, KeyModifiers},
    style::Color,
    style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{stdout, Write};

#[derive(Clone)]
pub struct Client {
    id: u32,
    position: Arc<RwLock<Position>>,
    old_position: Arc<RwLock<Position>>,
    score: i32,
    sender: Option<Sender<String>>,
}

impl Client {
    pub fn new(id: u32, position: Position, sender: Option<Sender<String>>) -> Self {
        Self {
            id,
            position: Arc::new(RwLock::new(position.clone())),
            old_position: Arc::new(RwLock::new(position)),
            score: 0,
            sender,
        }
    }

    pub fn adjust_score(&mut self, score: i32) {
        self.score += score;
    }

    pub fn set_position(&mut self, position: Position) {
        *self.position.write() = position;
    }

    pub fn set_old_position(&mut self, position: Position) {
        *self.old_position.write() = position;
    }
}

pub struct Game {
    clients: RwLock<Vec<Client>>,
    max_clients: u16,
    field: Field,
}

impl Drop for Game {
    fn drop(&mut self) {}
}

impl Game {
    pub fn new(width: u16, height: u16, max_clients: u16) -> Self {
        Self {
            clients: RwLock::new(Vec::new()),
            max_clients,
            field: Field { width, height },
        }
    }

    pub fn add_client(&self, client: Client) {
        self.clients.write().push(client);
    }

    pub fn remove_client(&self, client: &Client) {
        self.clients.write().retain(|c| c.id != client.id);
    }

    pub fn get_total_clients(&self) -> u32 {
        self.clients.read().len() as u32
    }

    pub fn create_random_position(&self) -> Position {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0..self.field.width) as u16;
        let y = rng.gen_range(0..self.field.height) as u16;
        Position { x, y }
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
        client.set_old_position(position.clone());
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

        // collect all positions
        let mut map: HashMap<Position, Vec<&Client>> = HashMap::new();
        for client in clients.iter() {
            let position = client.position.read();
            let entry = map.entry(position.clone()).or_insert(Vec::new());
            entry.push(client);
        }

        // print all clients
        for client in clients.iter() {
            let position = client.position.read();
            let old_position = client.old_position.read();
            queue!(
                stdout,
                cursor::MoveTo(position.x * 2, position.y),
                SetForegroundColor(Color::Red),
                Print(client.id.to_string()),
                ResetColor
            )
            .unwrap();

            // restore old position if there is no client
            if map.get(&old_position).is_none() {
                queue!(
                    stdout,
                    cursor::MoveTo(old_position.x * 2, old_position.y),
                    Print("C")
                )
                .unwrap();
            }
        }
        stdout.flush().unwrap();
    }

    pub async fn run(&self) {
        self.display_field_once();
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            self.refresh_field();
            let mut clients;
            {
                let mut map: HashMap<Position, Vec<&mut Client>> = HashMap::new();

                clients = self.clients.write().clone();

                for client in clients.iter_mut() {
                    let position = client.position.clone();
                    let entry = map.entry(position.read().clone()).or_insert(Vec::new());
                    entry.push(client);
                }

                for (_, clients) in map.iter_mut() {
                    if clients.len() < 2 {
                        for client in clients.iter_mut() {
                            client.adjust_score(1);
                        }
                        continue;
                    }
                    for client in clients.iter_mut() {
                        client.adjust_score(-5);
                        //move the client to a random position
                        let old_position = client.position.read().clone();
                        let new_position = self.create_random_position();
                        client.set_old_position(old_position);
                        client.set_position(new_position);
                    }
                }
            }

            for client in clients.iter() {
                //send a position update to the client
                let msg = Message::new_position(client.position.read().clone());
                if let Ok(msg) = msg.to_json() {
                    if let Some(sender) = &client.sender {
                        let _ = sender.send(msg).await;
                    }
                }
            }
        }
    }
}
