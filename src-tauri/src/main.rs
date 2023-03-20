#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::any::type_name;

use crate::warp_runner::ui_adapter::ChatAdapter;
use crossbeam::channel;
use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;
use std::pin::Pin;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use tokio::runtime::Handle;
pub mod config;
// pub mod utils;
use futures::channel::oneshot;
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::Duration;
use tauri::Manager;
use tokio::time::sleep;
use warp::error::Error;
mod warp_runner;
use crate::warp_runner::{
    ui_adapter::{MessageEvent, MultiPassEvent, RayGunEvent},
    ConstellationCmd, MultiPassCmd, RayGunCmd, WarpCmd, WarpCmdChannels, WarpEvent,
    WarpEventChannels,
};
use std::collections::HashMap;
use uuid::{uuid, Uuid};
use warp::crypto::DID;
mod state;
use crate::state::friends;
use crate::state::storage;
use crate::state::Chat;
use once_cell::sync::Lazy;
use state::State;
use std::sync::Arc;
mod testing;
use ::function_name::named;
use clap::Parser;
use std::path::PathBuf;

// ---- START WARP REQS
pub static WARP_CMD_CH: Lazy<WarpCmdChannels> = Lazy::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    WarpCmdChannels {
        tx,
        rx: Arc::new(tokio::sync::Mutex::new(rx)),
    }
});

// allows the UI to receive events from Warp
// pretty sure the rx channel needs to be in a mutex in order for it to be a static mutable variable
pub static WARP_EVENT_CH: Lazy<WarpEventChannels> = Lazy::new(|| {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    WarpEventChannels {
        tx,
        rx: Arc::new(tokio::sync::Mutex::new(rx)),
    }
});

#[derive(clap::Subcommand, Debug)]
enum LogProfile {
    /// normal operation
    Normal,
    /// print everything but tracing logs to the terminal
    Debug,
    /// print everything including tracing logs to the terminal
    Trace,
}
#[derive(Debug)]
pub struct StaticArgs {
    pub uplink_path: PathBuf,
    pub light_path: PathBuf,
    pub cache_path: PathBuf,
    pub config_path: PathBuf,
    pub warp_path: PathBuf,
    pub logger_path: PathBuf,
    pub tesseract_path: PathBuf,
    pub use_mock: bool,
    pub mock_cache_path: PathBuf,
    pub id_path: PathBuf,
    pub experimental: bool,
    pub login_config_path: PathBuf,
}
#[derive(Debug, Parser)]
#[clap(name = "")]
struct Args {
    /// The location to store the .uplink directory, within which a .warp, state.json, and other useful logs will be located
    #[clap(long)]
    path: Option<PathBuf>,
    #[clap(long)]
    experimental_node: bool,
    // todo: when the app is mature, default mock to false. also hide it behind a #[cfg(debug_assertions)]
    // there's no way to set --flag=true so for make the flag mean false
    /// mock data is fake friends, conversations, and messages, which allow for testing the UI.
    /// may cause crashes when attempting to add/remove fake friends, send messages to them, etc.
    #[clap(long, default_value_t = false)]
    no_mock: bool,
    /// configures log output
    #[command(subcommand)]
    profile: Option<LogProfile>,
}

pub static STATIC_ARGS: Lazy<StaticArgs> = Lazy::new(|| {
    let args = Args::parse();
    let light_path = match args.path {
        Some(path) => path,
        _ => dirs::home_dir().unwrap_or_default().join(".light"),
    };
    let warp_path = light_path.join("warp");
    StaticArgs {
        uplink_path: light_path.clone(),
        light_path: light_path.clone(),
        cache_path: light_path.join("state.json"),
        config_path: light_path.join("Config.json"),
        warp_path: light_path.join("warp"),
        tesseract_path: warp_path.join("tesseract.json"),
        id_path: light_path.join("warp/.id"),
        logger_path: light_path.join("debug.log"),
        mock_cache_path: light_path.join("mock-state.json"),
        use_mock: args.no_mock, // remove the ! to disable mock data
        experimental: args.experimental_node,
        login_config_path: light_path.join("login_config.json"),
    }
});
// --- END WARP REQS

pub struct StateState(Arc<Mutex<Option<State>>>);

impl State {
    fn accept(
        mut self,
        command: String,
        string_val_one: Option<String>,
        string_val_two: Option<String>,
        bool_val_one: Option<bool>,
        int_val_one: Option<i32>,
    ) -> state::State {
        // CREATE IDENTITY
        if command == "create_identity_command" {
            let handle = Handle::current();

            let (tx, rx): (Sender<bool>, Receiver<bool>) = channel();
            handle.spawn(async move {
                // string_val_one == username, string_val_two == password
                let outcome =
                    create_identity(string_val_one.unwrap(), string_val_two.unwrap()).await;
                tx.send(outcome).unwrap();
            });
            self.identity_exists = rx.recv().unwrap();
            self.logged_in = true;
        } else if command == "increment_counter_command" {
            self.counter = self.counter + int_val_one.unwrap();
        } else if command == "login_command"
        // LOGIN
        {
            let handle = Handle::current();

            let (tx, rx): (Sender<bool>, Receiver<bool>) = channel();
            let (tx2, rx2): (
                Sender<(state::friends::Friends, HashSet<state::identity::Identity>)>,
                Receiver<(state::friends::Friends, HashSet<state::identity::Identity>)>,
            ) = channel();
            let (tx3, rx3): (Sender<(HashMap<Uuid, state::Chat>, HashSet<state::Identity>)>, Receiver<(HashMap<Uuid, state::Chat>, HashSet<state::Identity>)>) =
                channel();
            let (tx4, rx4): (
                Sender<state::storage::Storage>,
                Receiver<state::storage::Storage>,
            ) = channel();
            handle.spawn(async move {
                // string_val_one == password
                let res = try_login(string_val_one.unwrap()).await;

                match res {
                    Ok(_) => {
                        if res.as_ref().unwrap() == &true {
                            let friends_tuple = initialize_friends().await;
                            let conversations_tuple = initialize_conversations().await;
                            let storage = initialize_files().await;

                            //      - think about passing the whole state struct to the front end ? json, deserialize needed serde ??

                            tx.send(res.unwrap()).unwrap();
                            tx2.send(friends_tuple).unwrap();
                            tx3.send(conversations_tuple).unwrap();
                            tx4.send(storage).unwrap();
                        }
                    }
                    // todo: notify user
                    Err(ref e) => {
                        println!("Failed with error: {:?}", e);
                        tx.send(false);
                    }
                }
            });
            let res = rx.recv().unwrap();
            match res {
                true => {
                    println!("Login successful.");
                    self.logged_in = true;
                }
                // todo: notify user
                false => {
                    println!("Failed with error ");
                }
            }

            let friends_tuple = rx2.recv().unwrap();

            self.set_friends(friends_tuple.0, friends_tuple.1);


            let conversations_tuple = rx3.recv().unwrap();
            // println!("conversation_tuple: {:?}", conversations_tuple.0);

            self.set_chats(conversations_tuple.0, conversations_tuple.1);


            
            self.chats.initialized = true;
            let storage = rx4.recv().unwrap();
            self.storage = storage;
        } else if command == "check_for_identity_command"
        // CHECK IF IDENTITY HAS BEEN CREATED
        {
            let b = std::path::Path::new(&STATIC_ARGS.id_path).exists();
            if b == false {
                self.identity_exists = false;
            } else if b == true {
                self.identity_exists = true;
            }
        }
        // DELETE IDENTITY
        else if command == "delete_identity_command" {
            std::fs::remove_dir_all(dirs::home_dir().unwrap_or_default().join(".light"));
            println!("{:?}", dirs::home_dir().unwrap_or_default().join(".light"));
            std::process::abort();
        }
        // SEND FRIEND REQUEST
        else if command == "send_friend_request_command" {
            let handle = Handle::current();

            let (tx, rx): (Sender<bool>, Receiver<bool>) = channel();
            handle.spawn(async move {
                // string_val_one == did_key
                let outcome = send_friend_request(string_val_one.unwrap()).await;
                tx.send(outcome).unwrap();
            });
            rx.recv().unwrap();
        }
        // SEND INITAL MESSAGE
        else if command == "send_initial_message_command" {
            let (tx, rx): (
                Sender<(state::Identity, HashMap<Uuid, state::chats::Chat>)>,
                Receiver<(state::Identity, HashMap<Uuid, state::chats::Chat>)>,
            ) = channel();
            let (tx2, rx2): (
                Sender<state::friends::Friends>,
                Receiver<state::friends::Friends>,
            ) = channel();
            let handle = Handle::current();
            handle.spawn(async move {
                // string_val_one == did_key
                let outcome = create_conversation(string_val_one.unwrap()).await;
                // println!("OUTCOME: {:?}", outcome.unwrap().id);
                println!("out-BULLFROG");
                // println!("string_val_two{:?}", string_val_two.unwrap());
                // println!("outcome{:?}", outcome.unwrap());
                // string_val_two == message
                let outcome_two =
                    send_message(string_val_two.unwrap(), outcome.unwrap().inner.id).await;

                // tx.send(outcome).unwrap();

                // string_val_two == message
                // let outcome_two = send_friend_request(string_val_two.unwrap()).await;
                // tx2.send(outcome_two).unwrap();
            });
            // rx.recv().unwrap();
            // rx2.recv().unwrap();
        } else if command == "send_message_command" {
            let handle = Handle::current();
            handle.spawn(async move {
                // string_val_one == message
                // string_val_two == conv_id
                let conv_id_uuid = Uuid::parse_str(&string_val_two.unwrap());
                let message = string_val_one.unwrap();
                // println!("{:?}", message);
                // println!("{:?}", conv_id_uuid.unwrap());

                let outcome_two = send_message(message, conv_id_uuid.unwrap()).await;

                // println!("OUTCOME TWO: {:?}", outcome_two);
                // tx.send(outcome).unwrap();

                // string_val_two == message
                // let outcome_two = send_friend_request(string_val_two.unwrap()).await;
                // tx2.send(outcome_two).unwrap();
            });
        }

        {
            state(self)
        }
    }
}

fn state(model: state::State) -> state::State {
    let model = next_action(model);
    return model;
}

fn next_action(model: state::State) -> state::State {
    // if model.counter < 100 {
    //     let step = 5;
    //     let new_model = model.accept(
    //         "increment_counter_command".to_string(),
    //         None,
    //         None,
    //         None,
    //         Some(5),
    //     );
    //     return new_model;
    // } else {
    //     return model;
    // }
    return model;
}

// TODO
// - get the state variable loaded when sam starts and replace Model with it

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
async fn main() {
    if fdlimit::raise_fd_limit().is_none() {}
    std::fs::create_dir_all(STATIC_ARGS.light_path.clone())
        .expect("Error creating Uplink directory");
    std::fs::create_dir_all(STATIC_ARGS.warp_path.clone()).expect("Error creating Warp directory");

    let state = Arc::new(Mutex::new(Some(state::State::load())));

    let state_clone = state.clone();

    let handle_warp_runner = || {
        let handle = Handle::current();
        handle.spawn(async move {
            let mut warp_instance = warp_runner::WarpRunner::new();
            warp_instance.run();
        });
    };

    tauri::Builder::default()
        .setup(move |app| {
            #[cfg(debug_assertions)] // only include this code on debug builds
            {
                let window = app.get_window("main").unwrap();
                window.open_devtools();
            }
            let app_handle = app.app_handle();

            app_handle.run_on_main_thread(handle_warp_runner);

            let app_handle_ref = app.app_handle();
            let handle_warp_events = move || {
                let handle = Handle::current();
                let state = state_clone;

                handle.spawn(async move {
                    let mut ch = WARP_EVENT_CH.rx.lock().await;
                    while let Some(evt) = ch.recv().await {
                        println!("in-BULLFROG");
                        state
                            .lock()
                            .unwrap()
                            .as_mut()
                            .unwrap()
                            .process_warp_event(evt);

                        app_handle_ref.emit_all("warp-event", &state).unwrap();
                    }
                });
            };

            app_handle.run_on_main_thread(handle_warp_events);

            Ok(())
        })
        .manage(StateState(state))
        .invoke_handler(tauri::generate_handler![
            start_sam_command,
            check_for_identity_command,
            increment_counter_command,
            login_command,
            create_identity_command,
            delete_identity_command,
            get_own_did_key_command,
            send_friend_request_command,
            send_initial_message_command,
            send_message_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn initialize_conversations() -> (HashMap<Uuid, Chat>, HashSet<state::identity::Identity>) {
    // Initialise conversations
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let res = loop {
        let (tx, rx) = oneshot::channel::<
            Result<(HashMap<Uuid, Chat>, HashSet<state::identity::Identity>), warp::error::Error>,
        >();
        warp_cmd_tx
            .send(WarpCmd::RayGun(RayGunCmd::InitializeConversations {
                rsp: tx,
            }))
            .expect("main failed to send warp command");

        match rx.await {
            Ok(r) => break r,
            Err(_e) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
        }
    };
    let conversation_tuple = res.unwrap();
    (conversation_tuple.0, conversation_tuple.1)
}

async fn initialize_friends() -> (state::friends::Friends, HashSet<state::identity::Identity>){
    // Initialize friends
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let (tx, rx) = oneshot::channel::<
        Result<(state::friends::Friends, HashSet<state::identity::Identity>), warp::error::Error>,
    >();
    warp_cmd_tx
        .send(WarpCmd::MultiPass(MultiPassCmd::InitializeFriends {
            rsp: tx,
        }))
        .expect("main failed to send warp command");

    let res = rx.await.expect("failed to get response from warp_runner");
    let friends = res.expect("Something broke");
    (friends.0, friends.1)
    // FIX HERE SO WE ALSO USE friends.1
}

async fn initialize_files() -> state::storage::Storage {
    // Initialize friends
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let (tx, rx) = oneshot::channel::<Result<storage::Storage, warp::error::Error>>();
    warp_cmd_tx
        .send(WarpCmd::Constellation(
            ConstellationCmd::GetItemsFromCurrentDirectory { rsp: tx },
        ))
        .expect("main failed to send warp command");

    let res = rx.await.expect("failed to get response from warp_runner");
    let storage = res.expect("Something broke");
    storage
}
async fn send_friend_request(did_key: String) -> bool {
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
    warp_cmd_tx
        .send(WarpCmd::MultiPass(MultiPassCmd::RequestFriend {
            rsp: tx,
            did: DID::from_str(&did_key).unwrap(),
        }))
        .expect("main failed to send warp command");
    let res = rx.await.expect("failed to get response from warp_runner");
    match res {
        Ok(_) | Err(Error::FriendRequestExist) => {
            println!("friend request ok?");
        }
        Err(e) => println!("Error: {:?}", e),
    }
    true
}

async fn create_conversation(did_key: String) -> Result<ChatAdapter, warp::error::Error> {
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let (tx, rx) = oneshot::channel::<Result<ChatAdapter, _>>();
    warp_cmd_tx
        .send(WarpCmd::RayGun(RayGunCmd::CreateConversation {
            rsp: tx,
            recipient: DID::from_str(&did_key).unwrap(),
        }))
        .expect("main failed to send warp command");
    let res = rx.await.expect("failed to get response from warp_runner");
    match res {
        Ok(_) | Err(Error::FriendRequestExist) => {
            println!("create conversation success");
            res
        }
        Err(e) => {
            println!("Error: {:?}", e);
            Err(e)
        }
    }
}

async fn send_message(message: String, conv_id: Uuid) -> Result<(), warp::error::Error> {
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    let (tx, rx) = oneshot::channel::<Result<(), warp::error::Error>>();
    let vec_string_message: Vec<String> =
        message.split("something").map(|s| s.to_string()).collect();

    let files_to_upload: Vec<PathBuf> = Vec::new();
    warp_cmd_tx
        .send(WarpCmd::RayGun(RayGunCmd::SendMessage {
            rsp: tx,
            conv_id: conv_id,
            attachments: files_to_upload,
            msg: vec_string_message,
        }))
        .expect("main failed to send warp command");
    let res = rx.await.expect("failed to get response from warp_runner");
    match res {
        Ok(_) | Err(Error::FriendRequestExist) => {
            println!(" Message Sent");
            res
        }
        Err(e) => {
            println!("Error: {:?}", e);
            Err(e)
        }
    }
}

#[named]
#[tauri::command]
fn start_sam_command(state: tauri::State<StateState>) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();

    let model = model.accept(function_name!().to_string(), None, None, None, None);

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);

    return model_clone;
    // let state_tuple = (
    //     model_clone.clone().id,
    //     model_clone.clone().route,
    //     model_clone.clone().chats,
    //     model_clone.clone().friends,
    // );

    // state_tuple
}

#[named]
#[tauri::command]
fn check_for_identity_command(state: tauri::State<StateState>) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(function_name!().to_string(), None, None, None, None);

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);
    return model_clone;
}

#[tauri::command]
fn get_own_did_key_command(state: tauri::State<StateState>) -> String {
    let handle = Handle::current();
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    handle.spawn(async move {
        let outcome = send_own_did_key_to_front_end().await;
        tx.send(outcome.unwrap()).unwrap();
    });
    rx.recv().unwrap()
}

#[named]
#[tauri::command]
fn send_friend_request_command(did_key: String, state: tauri::State<StateState>) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(
        function_name!().to_string(),
        Some(did_key),
        None,
        None,
        None,
    );

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);
    return model_clone;
}

#[named]
#[tauri::command]
fn create_identity_command(
    username: String,
    password: String,
    state: tauri::State<StateState>,
) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(
        function_name!().to_string(),
        Some(username),
        Some(password),
        None,
        None,
    );

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);
    return model_clone;
}

#[named]
#[tauri::command]
fn increment_counter_command(step: i32, state: tauri::State<StateState>) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(function_name!().to_string(), None, None, None, Some(step));

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);
    return model_clone;
}

#[named]
#[tauri::command]
fn delete_identity_command(state: tauri::State<StateState>) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(function_name!().to_string(), None, None, None, None);

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);
    return model_clone;
}

#[named]
#[tauri::command]
fn login_command(password: String, state: tauri::State<StateState>) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(
        function_name!().to_string(),
        Some(password),
        None,
        None,
        None,
    );

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    println!("chats after logging in: {:?}", model_clone.chats);
    *state_guard = Some(model);

    return model_clone;
}

#[named]
#[tauri::command]
fn send_initial_message_command(
    did_key: String,
    message: String,
    state: tauri::State<StateState>,
) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(
        function_name!().to_string(),
        Some(did_key),
        Some(message),
        None,
        None,
    );

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);
    return model_clone;
}

#[named]
#[tauri::command]
fn send_message_command(
    conv_id: String,
    message: String,
    state: tauri::State<StateState>,
) -> state::State {
    let mut model = state.0.lock().unwrap().take().unwrap();
    let model = model.accept(
        function_name!().to_string(),
        Some(conv_id),
        Some(message),
        None,
        None,
    );

    let mut state_guard = state.0.lock().unwrap();
    let model_clone = model.clone();
    *state_guard = Some(model);
    return model_clone;
}

async fn try_login(passphrase: String) -> Result<bool, Error> {
    // Try Login
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();

    let (tx, rx) =
        oneshot::channel::<Result<warp::multipass::identity::Identity, warp::error::Error>>();
    warp_cmd_tx
        .send(WarpCmd::MultiPass(MultiPassCmd::TryLogIn {
            passphrase,
            rsp: tx,
        }))
        .expect("UnlockLayout failed to send warp command");

    let res = rx.await.expect("failed to get response from warp_runner");

    match res {
        Ok(_) => {
            println!("Login worked");
            Ok(true)
        }
        // todo: notify user
        Err(e) => {
            println!("Login Failed {:?}", e);
            Err(e)
        }
    }
}

async fn send_own_did_key_to_front_end() -> Result<String, Error> {
    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    // Get own did:key
    let (tx, rx) = oneshot::channel::<Result<DID, warp::error::Error>>();
    if let Err(e) = warp_cmd_tx.send(WarpCmd::MultiPass(MultiPassCmd::GetOwnDid { rsp: tx })) {
        log::error!("failed to send warp command: {}", e);
    }

    let res = rx.await.expect("failed to get response from warp_runner");

    match res {
        Ok(_) => Ok(res.unwrap().to_string()),
        // todo: notify user
        Err(e) => Err(e),
    }
}

async fn create_identity(username: String, passphrase: String) -> bool {
    // Create Identity
    let (tx, rx) =
        oneshot::channel::<Result<warp::multipass::identity::Identity, warp::error::Error>>();

    let warp_cmd_tx = WARP_CMD_CH.tx.clone();
    warp_cmd_tx
        .send(WarpCmd::MultiPass(MultiPassCmd::CreateIdentity {
            username,
            passphrase,
            rsp: tx,
        }))
        .expect("UnlockLayout failed to send warp command");

    let res = rx.await.expect("failed to get response from warp_runner");

    match res {
        Ok(_) => {
            println!("Create identity successful.");
            true
        }
        Err(e) => {
            println!("Create identity failed {:?}", e);
            false
        }
    }
}

fn process_multipass_event(event: MultiPassEvent) {
    match event {
        MultiPassEvent::None => {}
        MultiPassEvent::FriendRequestReceived(identity) => {
            // self.friends.incoming_requests.insert(identity.clone());

            // self.mutate(Action::AddNotification(
            //     notifications::NotificationKind::FriendRequest,
            //     1,
            // ));

            // // TODO: Get state available in this scope.
            // // Dispatch notifications only when we're not already focused on the application.
            // let notifications_enabled = self
            //     .configuration
            //     .config
            //     .notifications
            //     .friends_notifications;

            // if !self.ui.metadata.focused && notifications_enabled {
            // crate::utils::notifications::push_notification(
            //     get_local_text("friends.new-request"),
            //     format!("{} sent a request.", identity.username()),
            //     Some(crate::utils::sounds::Sounds::Notification),
            //     notify_rust::Timeout::Milliseconds(4),
            // );
            // }
        }
        MultiPassEvent::FriendRequestSent(identity) => {
            // self.friends.outgoing_requests.insert(identity);
        }
        MultiPassEvent::FriendAdded(identity) => {
            // self.friends.incoming_requests.remove(&identity);
            // self.friends.outgoing_requests.remove(&identity);
            // self.friends.all.insert(identity.did_key(), identity);
        }
        MultiPassEvent::FriendRemoved(identity) => {
            // self.friends.all.remove(&identity.did_key());
        }
        MultiPassEvent::FriendRequestCancelled(identity) => {
            // self.friends.incoming_requests.remove(&identity);
            // self.friends.outgoing_requests.remove(&identity);
        }
        MultiPassEvent::FriendOnline(identity) => {
            // if let Some(ident) = self.friends.all.get_mut(&identity.did_key()) {
            // ident.set_identity_status(IdentityStatus::Online);
            // }
        }
        MultiPassEvent::FriendOffline(identity) => {
            // if let Some(ident) = self.friends.all.get_mut(&identity.did_key()) {
            // ident.set_identity_status(IdentityStatus::Offline);
            // }
        }
        MultiPassEvent::Blocked(identity) => {
            // self.block(&identity);
        }
        MultiPassEvent::Unblocked(identity) => {
            // self.unblock(&identity);
        }
        MultiPassEvent::IdentityUpdate(_) => todo!(),
    }
}
