use parking_lot::lock_api::RwLockWriteGuard;
use parking_lot::{RawRwLock, RwLock};
use rand::Rng;
use shared::*;
use std::collections::HashMap;
use std::collections::HashSet;
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
struct ClientState {
    id: u16,
    position: Position,
    old_position: Position,
    score: i32,
    color: Color,
}

#[derive(Clone, Debug)]
pub struct Client {
    id: u16,
    state: Arc<RwLock<ClientState>>,
    sender: Sender<String>,
}

impl Client {
    pub fn new(id: u16, position: Position, sender: Sender<String>, color: Color) -> Self {
        Self {
            id,
            state: Arc::new(RwLock::new(ClientState {
                id,
                position: position.clone(),
                old_position: position,
                score: 0,
                color,
            })),
            sender,
        }
    }

    pub fn adjust_score(&mut self, score: i32) {
        self.state.write().score += score;
    }

    pub fn set_position(&mut self, position: Position) {
        self.state.write().position = position;
    }

    pub fn set_old_position(&mut self, position: Position) {
        self.state.write().old_position = position;
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
        let mut state = client.state.write();
        state.old_position = state.position.clone();
        let mut position = &mut state.position;
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

    fn refresh_field(&self, states_gaurds: &Vec<RwLockWriteGuard<RawRwLock, ClientState>>) {
        let mut stdout = stdout();

        let mut positions = HashSet::new();
        for state in states_gaurds.iter() {
            let position = state.position.clone();
            queue!(
                stdout,
                cursor::MoveTo(position.x * 2, position.y),
                SetForegroundColor(state.color),
                Print(state.id.to_string()),
                ResetColor
            )
            .unwrap();
            positions.insert(position);
        }

        for state in states_gaurds.iter() {
            let old_position = &state.old_position;
            if !positions.contains(old_position) {
                queue!(
                    stdout,
                    cursor::MoveTo(old_position.x * 2, old_position.y),
                    Print("X")
                )
                .unwrap();
            }
        }

        // let mut positions: HashMap<(u16, u16), (u16, Color)> = HashMap::new();
        // for client in clients.iter() {
        //     let position = client.position.read();
        //     positions.insert((position.x, position.y), (client.id, client.color));
        // }
        // for i in 0..self.field.height {
        //     for j in 0..self.field.width {
        //         queue!(stdout, cursor::MoveTo(i * 2, j)).unwrap();
        //         if let Some((id, color)) = positions.get(&(i, j)) {
        //             queue!(
        //                 stdout,
        //                 SetForegroundColor(*color),
        //                 Print(id.to_string()),
        //                 ResetColor
        //             )
        //             .unwrap();
        //             continue;
        //         }
        //         queue!(stdout, Print("X"),).unwrap();
        //     }
        // }

        stdout.flush().unwrap();
    }

    pub async fn run(&self) {
        if self.running.read().clone() {
            return;
        }
        *self.running.write() = true;
        self.display_field_once();
        loop {
            tokio::time::sleep(Duration::from_millis(700)).await;
            {
                let clients = self.clients.write();

                let mut states_gaurds: Vec<RwLockWriteGuard<RawRwLock, ClientState>> =
                    clients.iter().map(|client| client.state.write()).collect();

                let mut map: HashMap<Position, Vec<&mut RwLockWriteGuard<RawRwLock, ClientState>>> =
                    HashMap::new();

                for state in states_gaurds.iter_mut() {
                    let position = state.position.clone();
                    let entry = map.entry(position).or_insert(Vec::new());
                    entry.push(state);
                }

                for (_, states) in map.iter_mut() {
                    if states.len() > 1 {
                        for state in states.iter_mut() {
                            state.score -= 5;
                            let new_position = self.create_random_position();
                            state.position = new_position;
                        }
                        continue;
                    }
                    for state in states.iter_mut() {
                        state.score += 1;
                    }
                }
                self.refresh_field(&states_gaurds);
            }

            for client in self.clients.read().iter() {
                let pos_msg = Message::new_position(client.state.read().position.clone())
                    .to_json()
                    .unwrap();
                let score_msg = Message::new_score(client.state.read().score.clone())
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
