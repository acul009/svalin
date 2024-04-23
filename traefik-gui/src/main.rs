#![allow(non_snake_case)]

use dioxus::prelude::*;
use log::LevelFilter;

#[cfg(feature = "server")]
mod db;
#[cfg(feature = "server")]
use db::DB;

#[derive(Clone, Routable, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum Route {
    #[route("/")]
    Home {},
    #[route("/blog/:id")]
    Blog { id: i32 },
}

fn main() {
    // Init debug
    dioxus_logger::init(LevelFilter::Info).expect("failed to init logger");

    launch(App);
}

fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

#[component]
fn Blog(id: i32) -> Element {
    rsx! {
        Link { to: Route::Home {}, "Go to counter" }
        "Blog post {id}"
    }
}

#[component]
fn Home() -> Element {
    let mut count = use_signal(|| 0);
    let mut text = use_signal(|| String::from("..."));

    rsx! {
        Link {
            to: Route::Blog {
                id: count()
            },
            "Go to blog"
        }
        div {
            h1 { "High-Five counter: {count}" }
            button { onclick: move |_| count += 1, "Up high!" }
            button { onclick: move |_| count -= 1, "Down low!" }
            button {
                onclick: move |_| async move {
                    if let Ok(data) = get_server_data().await {
                        log::info!("Client received: {}", data);
                        text.set(data.clone());
                        post_server_data(data).await.unwrap();
                    }
                },
                "Get Server Data"
            }
            p { "Server data: {text}"}
        }
    }
}

#[server(PostServerData)]
async fn post_server_data(data: String) -> Result<(), ServerFnError> {
    let scope = DB.scope("default".into())?;
    scope
        .update(|b| {
            b.put("data", data.clone())?;

            Ok(())
        })
        .unwrap();
    // println!("Server received: {}", data);
    Ok(())
}

#[server(GetServerData)]
async fn get_server_data() -> Result<String, ServerFnError> {
    let scope = DB.scope("default".into())?;
    let mut data = None;
    scope
        .view(|b| {
            if let Some(raw) = b.get_kv("data") {
                data = Some(String::from_utf8(raw.value().to_owned())?);
            }

            Ok(())
        })
        .unwrap();
    match data {
        Some(d) => Ok(format!("Hello with data: {}", d)),
        None => Ok("Hello without data".to_string()),
    }
}
