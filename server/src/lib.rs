use parking_lot::lock_api::RwLockWriteGuard;
use parking_lot::{RawRwLock, RwLock};
use rand::Rng;
use shared::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

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
    sender: UnboundedSender<String>,
}

impl Client {
    pub fn new(id: u16, position: Position, sender: UnboundedSender<String>, color: Color) -> Self {
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
    last_id: RwLock<u16>,
}

impl Game {
    pub fn new(width: u16, height: u16, max_clients: u16) -> Self {
        Self {
            clients: RwLock::new(Vec::new()),
            running: RwLock::new(false),
            max_clients,
            field: Field { width, height },
            last_id: RwLock::new(0),
        }
    }

    pub fn add_client(&self, client: Client) {
        self.place_client(&client);
        self.clients.write().push(client);
    }

    pub fn remove_client(&self, client: &Client) {
        let mut clients = self.clients.write();

        let state = client.state.read();
        let position = state.position.clone();
        let old_position = state.old_position.clone();

        clients.retain(|c| c.id != client.id);

        let mut positions = HashSet::new();
        for client in clients.iter() {
            positions.insert(client.state.read().position.clone());
        }

        let mut stdout = stdout();
        if !positions.contains(&position) {
            queue!(
                stdout,
                cursor::MoveTo(position.x * 2, position.y),
                Print("X"),
            )
            .unwrap();
        }
        if !positions.contains(&old_position) {
            queue!(
                stdout,
                cursor::MoveTo(old_position.x * 2, old_position.y),
                Print("X"),
            )
            .unwrap();
        }
        stdout.flush().unwrap();
    }

    pub fn create_id(&self) -> u16 {
        let mut id = self.last_id.write();
        *id += 1;
        if *id > 9 {
            let mut stdout = stdout();
            execute!(
                stdout,
                Clear(ClearType::All),
                cursor::MoveTo(0, 0),
                Print("Client id too big!\n")
            )
            .unwrap();
            std::process::exit(1);
        }
        *id
    }

    pub fn get_total_clients(&self) -> u16 {
        self.clients.read().len() as u16
    }

    pub fn get_max_clients(&self) -> u16 {
        self.max_clients
    }

    pub fn start_game(&self) -> bool {
        self.get_total_clients() >= self.get_max_clients()
    }

    fn place_client(&self, client: &Client) {
        let mut stdout = stdout();
        let state = client.state.read();
        execute!(
            stdout,
            cursor::MoveTo(state.position.x * 2, state.position.y),
            SetForegroundColor(state.color),
            Print(state.id.to_string()),
            ResetColor
        )
        .unwrap();
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

    pub fn display_field_once(&self) {
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

        stdout.flush().unwrap();
    }

    pub async fn run(&self) {
        if self.running.read().clone() {
            return;
        }
        *self.running.write() = true;
        loop {
            tokio::time::sleep(Duration::from_millis(700)).await;
            {
                let clients = self.clients.write();

                let mut states_gaurds: Vec<RwLockWriteGuard<RawRwLock, ClientState>> =
                    clients.iter().map(|client| client.state.write()).collect();

                self.refresh_field(&states_gaurds);

                let mut map: HashMap<Position, Vec<&mut RwLockWriteGuard<RawRwLock, ClientState>>> =
                    HashMap::new();

                for state in states_gaurds.iter_mut() {
                    let position = state.position.clone();
                    let entry = map.entry(position).or_insert(Vec::new());
                    entry.push(state);
                }

                for (_, states) in map.iter_mut() {
                    for state in states.iter_mut() {
                        state.old_position = state.position.clone();
                    }
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
            }

            for client in self.clients.read().iter() {
                let pos_msg = Message::new_position(client.state.read().position.clone())
                    .to_json()
                    .unwrap();
                let score_msg = Message::new_score(client.state.read().score.clone())
                    .to_json()
                    .unwrap();
                let sender = client.sender.clone();
                let _ = sender.send(pos_msg);
                let _ = sender.send(score_msg);
            }
        }
    }
}
