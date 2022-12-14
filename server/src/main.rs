use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::channel;
use poem::{
    get, handler,
    listener::TcpListener,
    web::{
        websocket::{Message, WebSocket},
        Data,
    },
    EndpointExt, IntoResponse, Route, Server,
};
use std::sync::Arc;
use server::*;



#[handler]
fn ws(ws: WebSocket, game: Data<&Arc<Game>>) -> impl IntoResponse {

    let id = game.get_total_clients();
    let position = game.create_random_position();
    let (tx, mut rx) = channel::<String>(100);
    let client = Client::new(id, position, Some(tx));

    let client_sender = client.clone();
    let mut client_receiver = client.clone();

    game.add_client(client);

    let game_sender = game.clone();
    let game_receiver = game.clone();

    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();

        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                if let Message::Text(text) = msg {
                    game_receiver.on_new_message(&mut client_receiver, text);
                }
            }
        });

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sink.send(Message::Text(msg)).await.is_err() {
                    game_sender.remove_client(&client_sender);
                    break;
                }
            }
        });
    })
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // if std::env::var_os("RUST_LOG").is_none() {
    //     std::env::set_var("RUST_LOG", "poem=debug");
    // }
    tracing_subscriber::fmt::init();

    let game = Arc::new(Game::new(3,3,10));

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
