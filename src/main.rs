#![feature(proc_macro_hygiene, decl_macro)]
#![feature(noop_waker)]
#[macro_use] extern crate rocket;

use reqwest::header::AUTHORIZATION;
use reqwest::Method;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::borrow::Cow;
use std::net::SocketAddr;
use rocket::{State, Request};
use rocket::tokio::sync::Mutex;
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::{Json, Value, json};
use rocket::futures::{SinkExt, StreamExt};
use tokio_util::io::StreamReader;
use futures::TryStreamExt;
use futures::AsyncBufReadExt;

struct TokenMapping {
    map: Mutex<HashMap<String,String>>
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct TokenReceipt<'r> {
    token: Cow<'r, str>
}


#[get("/echo?stream", rank = 1)]
fn echo_stream(ws: ws::WebSocket) -> ws::Stream!['static] {
    ws::Stream! { ws =>
        for await message in ws {
            yield message?;
        }
    }
}


#[get("/presocket", rank = 1)]
async fn pre(ws: ws::WebSocket, remote_addr: SocketAddr, tokens: &State<TokenMapping>) -> ws::Stream!['static] {
    let map = tokens.map.lock().await;
    let token = map[&remote_addr.ip().to_string()].clone();
    let client = reqwest::Client::new();
    let mut request = reqwest::Request::new(Method::GET,reqwest::Url::parse(&"https://lichess.org/api/stream/event").unwrap());
    request.headers_mut().insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    *request.timeout_mut() = None;
    let mut lines = client.execute(request).await
    .unwrap()
    .bytes_stream()
    .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
    .into_async_read()
    .lines();
    ws::Stream! { ws =>
        while let Some(v) = lines.next().await {
            match v {
                Ok(x) => yield ws::Message::text(x),
                Err(_) => continue
            }
        }
    }
}


#[get("/boardsocket", rank = 1)]
async fn board(ws: ws::WebSocket, remote_addr: SocketAddr, tokens: &State<TokenMapping>) -> ws::Stream!['static] {
    let map = tokens.map.lock().await;
    let token = map[&remote_addr.ip().to_string()].clone();
    let client = reqwest::Client::new();
    
    ws::Stream! {ws =>
        for await message in ws {
            let message2 = match message {
                Ok(x) => x,
                Err(_) => {
                    yield ws::Message::Close(Option::None);
                    return;
                }
            };
            let messagedata =  match message2.into_text() {
                Ok(x) => x,
                Err(_) => {
                    yield ws::Message::Close(Option::None);
                    return;
                }
            };
            let mut request = reqwest::Request::new(Method::GET,reqwest::Url::parse(&format!("https://lichess.org/api/board/game/stream/{}",messagedata)).unwrap());
            request.headers_mut().insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
            *request.timeout_mut() = None;
            let mut lines = client.execute(request).await
            .unwrap()
            .bytes_stream()
            .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
            .into_async_read()
            .lines();
            while let Some(v) = lines.next().await {
                match v {
                    Ok(x) => yield ws::Message::text(x),
                    Err(_) => continue
                }
            }
        }
    }
}

#[get("/play/<game>/<gmove>")]
async fn play(remote_addr: SocketAddr, game: &str ,gmove: &str,tokens: &State<TokenMapping>) -> String {
    let map = tokens.map.lock().await;
    let token = map[&remote_addr.ip().to_string()].clone();
    let client = reqwest::Client::new();
    let mut request = reqwest::Request::new(Method::GET,reqwest::Url::parse(&format!("https://lichess.org/api/board/game/{}/move/{}",game,gmove)).unwrap());
    request.headers_mut().insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    let mut response = client.execute(request).await.unwrap().text().await.unwrap();
    return response;
}

#[get("/test/<id>")]
async fn test(remote_addr: SocketAddr, id: &str,tokens: &State<TokenMapping>) -> String {
    
    let map = tokens.map.lock().await;
    let token = map[&remote_addr.ip().to_string()].clone();
    let client = reqwest::Client::new();
    let mut request = reqwest::Request::new(Method::GET,reqwest::Url::parse(&format!("https://lichess.org/api/board/game/stream/{}",id)).unwrap());
    request.headers_mut().insert(AUTHORIZATION, format!("Bearer {}", token).parse().unwrap());
    *request.timeout_mut() = None;
    let line = client.execute(request).await
    .unwrap()
    .bytes_stream()
    .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
    .into_async_read()
    .lines().next().await;
    match line {
        Option::None => return "shit".to_owned(),
        Some(x) => {
            match x {
                Ok(z) => return z,
                Err(_) => return "poop".to_owned(),
            }
        }
    }
}

#[get("/")]
async fn index(remote_addr: SocketAddr,tokens: &State<TokenMapping>) -> String {
    println!("{}", remote_addr.ip().to_string());
    let map = tokens.map.lock().await;
    map[&remote_addr.ip().to_string()].clone()
}


#[post("/auth", format = "application/json", data = "<token>")]
async fn auth(remote_addr: SocketAddr, token: Json<TokenReceipt<'_>>, tokens: &State<TokenMapping>) -> Value {
    println!("{}", token.token.to_string());
    println!("{}", remote_addr.ip().to_string());
    insert_token(&token.token.to_string(),&remote_addr.ip().to_string(),tokens).await;
    json!({ "status": "ok"})
}

async fn insert_token(token:&str, remote_addr:&str, mapping: &State<TokenMapping>) {
    let mut map = mapping.map.lock().await;
    map.insert(remote_addr.to_owned(),token.to_owned());
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index])
    .mount("/", routes![auth])
    .mount("/", routes![echo_stream])
    .mount("/", routes![test])
    .mount("/", routes![play])
    .mount("/", routes![board])
    .mount("/", routes![pre])
    .manage(TokenMapping { map: Mutex::new(HashMap::<String,String>::new())})
}