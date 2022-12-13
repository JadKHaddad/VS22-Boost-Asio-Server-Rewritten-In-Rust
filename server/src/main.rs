use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use rand::Rng;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Sender;

use poem::{
    get, handler,
    listener::TcpListener,
    web::{
        websocket::{Message, WebSocket},
        Data,
    },
    EndpointExt, IntoResponse, Route, Server,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Eq, Hash, PartialEq)]
struct Position {
    x: u32,
    y: u32,
}

#[derive(Clone)]
struct Client {
    id: u32,
    position: Position,
    score: i32,
    sender: Option<Sender<String>>,
}

impl Client {
    fn new(id: u32, position: Position, sender: Option<Sender<String>>) -> Self {
        Self {
            id,
            position,
            score: 0,
            sender,
        }
    }

    fn adjust_score(&mut self, score: i32) {
        self.score += score;
    }
}

struct Game {
    clients: RwLock<Vec<Client>>,
}

impl Game {
    fn new() -> Self {
        Self {
            clients: RwLock::new(Vec::new()),
        }
    }

    fn add_client(&self, client: Client) {
        println!("adding client: {}", client.id);
        self.clients.write().push(client);
    }

    fn remove_client(&self, client: &Client) {
        self.clients.write().retain(|c| c.id != client.id);
        println!("client removed: {}", client.id);
    }

    fn get_total_clients(&self) -> u32 {
        self.clients.read().len() as u32
    }

    fn create_random_position(&self) -> Position {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0..100);
        let y = rng.gen_range(0..100);
        Position { x, y }
    }

    async fn run(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let mut clients;
            {
                let mut map: HashMap<Position, Vec<&mut Client>> = HashMap::new();

                clients = self.clients.write().clone();

                for client in clients.iter_mut() {
                    let position = client.position.clone();
                    let entry = map.entry(position).or_insert(Vec::new());
                    entry.push(client);
                }

                for (_, clients) in map.iter_mut() {
                    if clients.len() < 1 {
                        for client in clients.iter_mut() {
                            client.adjust_score(1);
                        }
                        continue;
                    }
                    for client in clients.iter_mut() {
                        client.adjust_score(-5);
                    }
                }
            }
            for client in clients.iter() {
                let position = client.position.clone();
                let msg = format!("{} {}", position.x, position.y);
                if let Some(sender) = &client.sender {
                    println!("sending: {}", msg);
                    let _ = sender.send(msg).await;
                }
            }
        }
    }
}

#[handler]
fn ws(ws: WebSocket, game: Data<&Arc<Game>>) -> impl IntoResponse {
    let game = game.clone();
    let id = game.get_total_clients();
    let position = game.create_random_position();
    let (tx, mut rx) = channel::<String>(100);
    let client = Client::new(id, position, Some(tx));

    let client_c = client.clone();

    game.add_client(client);

    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();

        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(text) = msg {
                    println!("received: {}", text);
                }
            }
        });

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sink.send(Message::Text(msg)).await.is_err() {
                    game.remove_client(&client_c);
                    break;
                }
            }
        });
    })
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }
    tracing_subscriber::fmt::init();

    let game = Arc::new(Game::new());

    // run game
    let game_c = game.clone();
    tokio::spawn(async move {
        game_c.run().await;
    });

    let app = Route::new().at("/", get(ws.data(game)));

    Server::new(TcpListener::bind("127.0.0.1:3000"))
        .run(app)
        .await
}
