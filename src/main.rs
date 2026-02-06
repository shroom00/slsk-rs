#[macro_use]
mod macros;
mod config;
mod constants;
mod events;
mod gui;
mod messages;
mod packing;
mod parsers;
pub(crate) mod peer_handling;
pub(crate) mod server_handling;
mod sql;
#[allow(dead_code)]
mod styles;
mod utils;
pub(crate) mod file_transfer;

use crate::config::{Config, CONFIG_PATH};
use crate::constants::{DownloadStatus, Percentage};
use crate::events::SLSKEvents;
use crate::messages::*;
use crate::packing::UnpackFromBytes;
use crate::peer_handling::{start_listener_task, start_peer_task};
use crate::server_handling::{start_server_read_task, start_server_write_task};
use crate::sql::DiskIndex;
use crate::utils::keepalive_add_retries;

use constants::{ConnectionTypes, TransferDirections, MAX_RESULTS};
use crossbeam_deque::Worker;
use gui::widgets::table;
use serde::Deserialize;
use smol::Timer;
use socket2::{SockRef, TcpKeepalive};
use std::collections::{HashMap, VecDeque};
use std::io::{read_to_string, Error, ErrorKind, Write};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use std::thread::{self};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::RwLock;
use tokio::sync::{broadcast::channel, Mutex};
use tokio::task::JoinError;

const QUEUE_SIZE: usize = 1_000;
const CHUNK_SIZE: usize = 500_000; // half a MB
const CONNECTION_TIME: u64 = 5;
const LOGGING_ENABLED: bool = false;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_BACKTRACE", "1");

    let mut config: Config = Config {
        server: Default::default(),
        user: Default::default(),
        index: DiskIndex::new(".shares").await?,
    };

    let config_path = Path::new(CONFIG_PATH);

    // write the config to a file if it doesn't exist
    if !config.write_to_file(config_path, false) {
        // otherwise load it from the file
        config = Config::read_from_file(config_path).expect("Failed to read config from file");
        let old_config = read_to_string(std::fs::File::open(config_path)?)?;

        // if the shares database doesn't exist, the new config won't have any shares.
        // we get around this by using the shares from the old index.
        if config.index.root_folders().is_empty() {
            if let Ok(old_index) =
                Config::deserialize(toml::Deserializer::new(&old_config)).map(|c| c.index)
            {
                config.index = old_index;
            }
        }
        let new_config = toml::to_string(&config)?;
        // if the config from the file isn't the same as the one in memory, backup + overwrite the old config
        if new_config != old_config {
            std::fs::File::options()
                .create(true)
                .write(true)
                .truncate(true)
                .open(format!("{}.old", config_path.to_string_lossy()))?
                .write_all(old_config.as_bytes())?;
            config.write_to_file(config_path, true);
        }
    }

    let shares_message = Arc::new(RwLock::new(None));

    // update the file index in the background
    // this stops the client freezing for ages while the files are being indexed for the first time
    tokio::task::spawn({
        let mut index = config.index.clone();
        let shares_message = Arc::clone(&shares_message);
        async move {
            let _ = index.reindex_all().await;
            *shares_message.write().await = Some(match index.file_list().await {
                Ok(file_list) => SharedFileListResponse::to_bytes(file_list),
                Err(_) => Vec::new(),
            });
        }
    });

    let config = Arc::new(RwLock::new(config));
    let gui_config = Arc::clone(&config);
    let connection_config = Arc::clone(&config);

    let (write_queue, read_queue) = channel::<SLSKEvents>(QUEUE_SIZE);
    let gui_read_queue = read_queue.resubscribe();
    let gui_write_queue = write_queue.clone();

    thread::spawn(
        move || match gui::main(gui_write_queue, gui_read_queue, gui_config) {
            Ok(()) => {
                return;
            }
            Err(e) => panic!("{e}"),
        },
    );
    let mut login_timeout: u64 = 15;

    let mut temp_receiver = read_queue.resubscribe();

    loop {
        if !'outer: loop {
            let should_restart = async {
                let mut quit = None;
                while quit.is_none() {
                    let event = temp_receiver.recv().await;
                    quit = match event {
                        Ok(event) => match event {
                            SLSKEvents::Quit { restart } => Some(restart),
                            _ => None,
                        },
                        Err(_) => None,
                    };
                }
                quit
            };

            let server_address = { connection_config.read().await.server.to_string() };

            // TODO: Make initial connection more robust, handling disconnection and displaying info in the UI properly (rather than printing)
            let stream: Result<TcpStream, Result<bool, Error>> = tokio::select! {
                // Wait for the connection to complete
                connect_result = TcpStream::connect(server_address) => match connect_result {
                    Ok(connect_result) => Ok(connect_result),
                    Err(e) => Err(Err(e)),
                },
                // Wait for should_restart to complete
                restart = should_restart => {
                    if let Some(restart) = restart {
                        Err(Ok(restart))
                    } else {unimplemented!()}
                }
            };
            match stream {
                Ok(stream) => {
                    let sock_ref = SockRef::from(&stream);
                    // These specific settings are based on nicotine+'s settings
                    let mut ka = TcpKeepalive::new()
                        .with_time(Duration::from_secs(10))
                        .with_interval(Duration::from_secs(2));
                    ka = keepalive_add_retries(ka);
                    sock_ref.set_tcp_keepalive(&ka)?;
                    // TODO: Set the number of TCP pings allowed before the connection is assumed to be dead (should be 10)
                    // on platforms where it's not supported by socket2 (e.g. Windows)
                    // Not 100% sure this is actually possible as it's an OS limitation, but I imagine it could be implemented manually.

                    let handle = tokio::spawn(handle_client(
                        stream,
                        write_queue.clone(),
                        read_queue.resubscribe(),
                        Arc::clone(&config),
                        Arc::clone(&shares_message),
                    ));

                    match handle.await {
                        Ok(slskexit) => match slskexit {
                            SLSKExitCode::LoginFail => (),
                            SLSKExitCode::IoError(e) => {
                                if e.kind() == ErrorKind::AddrInUse {
                                    panic!("The port you tried to use is already in use. You should edit the config file, or free the port.");
                                } else {
                                    panic!("{e}");
                                }
                            }
                            _ => {
                                break true;
                            }
                        },
                        Err(e) => {
                            panic!("{e}");
                        }
                    };
                }
                Err(Ok(restart)) => break 'outer restart,
                Err(Err(e)) => {
                    if e.raw_os_error() == Some(11001) {
                        // If "no such host is known" (Not connected to the internet?)
                        continue;
                    }
                    println!(
                        "stream fail: {e}, sleeping for {login_timeout} seconds. {:?}",
                        e
                    );
                    Timer::after(Duration::from_secs(login_timeout)).await;
                    println!("sleep finished");
                    login_timeout *= 2;
                }
            };
        } {
            break;
        }
    }
    Ok(())
}

#[allow(dead_code)]
#[derive(Debug)]
enum SLSKExitCode {
    Ok,
    LoginFail,
    Restart,
    JoinError(JoinError),
    OtherLogin,
    IoError(Error),
}

async fn handle_client(
    stream: TcpStream,
    write_queue: Sender<SLSKEvents>,
    read_queue: Receiver<SLSKEvents>,
    config: Arc<RwLock<Config>>,
    shares_message: Arc<RwLock<Option<Vec<u8>>>>,
) -> SLSKExitCode {
    let (reader, writer) = stream.into_split();

    let port = config.read().await.user.port;
    let listener = match TcpListener::bind(format!("0.0.0.0:{port}")).await {
        Ok(listener) => listener,
        Err(e) => return SLSKExitCode::IoError(e),
    };

    let my_port: u32 = listener.local_addr().unwrap().port().into();
    let my_username = Arc::new(RwLock::new(None));
    let server_my_username = Arc::clone(&my_username);
    let config_username = config.read().await.user.name.clone();

    let quit = Arc::new(RwLock::new(false));
    // We have to clone the quit flag so it can be read in different tokio tasks
    let quit_write = Arc::clone(&quit);

    let logged_in = Arc::new(RwLock::new(false));
    // The listener needs to know if we're logged in so it can ignore connections we may receive from previous sessions.
    // This can happen if you logout and login in quick succession.
    let logged_in_listener = Arc::clone(&logged_in);

    let indirect_peers_list_writer = Worker::<_ReceiveConnectToPeer>::new_fifo();
    let direct_peers_list_writer = Worker::<(TcpStream, String, u32, ConnectionTypes)>::new_fifo();
    let prompted_peers_list_writer = Worker::<(String, u32, ConnectionTypes)>::new_fifo();
    let indirect_peers_list_reader = indirect_peers_list_writer.stealer();
    let direct_peers_list_reader = direct_peers_list_writer.stealer();
    let prompted_peers_list_reader = prompted_peers_list_writer.stealer();
    let peer_write_queue = write_queue.clone();
    let writer_write_queue = write_queue.clone();

    let file_info_map = Arc::new(Mutex::new(HashMap::<u32, VecDeque<(String, u64)>>::new()));

    let user_info_map = Arc::new(Mutex::new(HashMap::<String, (Ipv4Addr, u32)>::new()));
    let peer_user_info_map = Arc::clone(&user_info_map);
    let writer_user_info_map = Arc::clone(&user_info_map);

    let token_message_map = Arc::new(Mutex::new(HashMap::<u32, VecDeque<Vec<u8>>>::new()));
    let peer_token_message_map = Arc::clone(&token_message_map);

    let download_filename_map = Arc::new(Mutex::new(HashMap::<
        String,
        VecDeque<(
            Arc<RwLock<DownloadStatus>>,
            Arc<RwLock<Percentage>>,
            Option<bool>,
        )>,
    >::new()));
    let peer_download_filename_map = Arc::clone(&download_filename_map);

    // Spawn separate tasks for reading and writing
    let server_read_task = start_server_read_task(
        quit,
        logged_in,
        reader,
        write_queue,
        indirect_peers_list_writer,
        server_my_username,
        config_username,
        user_info_map,
    )
    .await;

    let server_write_task = start_server_write_task(
        quit_write,
        config,
        read_queue,
        writer,
        my_username.clone(),
        my_port,
        token_message_map,
        writer_user_info_map,
        writer_write_queue,
        prompted_peers_list_writer,
        download_filename_map,
    )
    .await;

    let listener_task =
        start_listener_task(listener, logged_in_listener, direct_peers_list_writer).await;

    // TODO: Handle old peers better
    // Peers who try to send data from a previous search (now deleted/invalid) still get sent to the queue.
    // This slows down receiving data that is actually desired.
    // To combat this, there should be some list of currently valid tokens,
    // if data is then sent with an invalid token, the connection should be closed/the data should be ignored.
    let my_username = my_username;
    let peer_task = start_peer_task(
        prompted_peers_list_reader,
        indirect_peers_list_reader,
        direct_peers_list_reader,
        peer_user_info_map,
        my_username,
        peer_write_queue,
        peer_token_message_map,
        file_info_map,
        peer_download_filename_map,
        shares_message,
    )
    .await;

    let read_result = server_read_task.await;
    match read_result {
        Ok(exit) => match exit {
            SLSKExitCode::LoginFail => {
                peer_task.abort();
                server_write_task.abort();
                listener_task.abort();
                return SLSKExitCode::LoginFail;
            }
            _ => (),
        },
        Err(_) => (),
    }

    let write_result = server_write_task.await;
    match write_result {
        Ok(exit) => {
            println!("handle_client finished!");
            exit
        }
        Err(e) => SLSKExitCode::JoinError(e),
    }
}
