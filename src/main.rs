#[macro_use]
mod macros;
mod constants;
mod events;
mod gui;
mod messages;
mod packing;
#[allow(dead_code)]
mod styles;
mod utils;

use crate::constants::{DownloadStatus, Percentage};
use crate::events::SLSKEvents;
use crate::messages::*;
use crate::packing::UnpackFromBytes;
use crate::utils::keepalive_add_retries;

use constants::{ConnectionTypes, TransferDirections, MAX_RESULTS};
use crossbeam_deque::Worker;
use flate2::read::ZlibDecoder;
use gui::widgets::table;
use smol::{block_on, Timer};
use socket2::{SockRef, TcpKeepalive};
use std::collections::{HashMap, VecDeque};
use std::fs::{create_dir, create_dir_all};
use std::io::{Error, ErrorKind, Read, Write};
use std::net::Ipv4Addr;
use std::path::Path;
use std::sync::Arc;
use std::thread::{self};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::RwLock;
use tokio::sync::{broadcast::channel, Mutex};
use tokio::task::JoinError;
use tokio::time::sleep;
use utils::log;

const QUEUE_SIZE: usize = 1_000;
const CHUNK_SIZE: usize = 500_000; // half a MB
const CONNECTION_TIME: u64 = 5;
const LOGGING_ENABLED: bool = false;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_BACKTRACE", "1");
    let (write_queue, read_queue) = channel::<SLSKEvents>(QUEUE_SIZE);
    let gui_read_queue = read_queue.resubscribe();
    let gui_write_queue = write_queue.clone();

    thread::spawn(move || match gui::main(gui_write_queue, gui_read_queue) {
        Ok(()) => {
            return;
        }
        Err(e) => panic!("{e}"),
    });
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

            // TODO: Make initial connection more robust, handling disconnection and displaying info in the UI properly (rather than printing)
            let stream: Result<TcpStream, Result<bool, Error>> = tokio::select! {
                // Wait for the connection to complete
                connect_result = TcpStream::connect("server.slsknet.org:2242") => match connect_result {
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
                    ));

                    match handle.await {
                        Ok(slskexit) => match slskexit {
                            SLSKExitCode::LoginFail => (),
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
    mut read_queue: Receiver<SLSKEvents>,
) -> SLSKExitCode {
    let (mut reader, mut writer) = stream.into_split();

    let port = 0;
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();

    let my_port: u32 = listener.local_addr().unwrap().port().into();
    let my_username = Arc::new(Mutex::new(None));

    let quit = Arc::new(RwLock::new(false));
    // We have to clone the quit flag so it can be read in different tokio tasks
    let quit_write = Arc::clone(&quit);

    let logged_in = Arc::new(Mutex::new(false));
    // The listener needs to know if we're logged in so it can ignore connections we may receive from previous sessions.
    // This can happen if you logout and login in quick succession.
    let logged_in_listener = Arc::new(Mutex::new(false));

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

    // TODO: Make this use chunks + change usage to account for this. (?)
    async fn get_code_and_bytes_from_readable<R>(
        reader: &mut R,
        message_type: MessageType,
    ) -> Result<(MessageType, Vec<u8>), SLSKExitCode>
    where
        R: AsyncReadExt + Unpin,
    {
        let mut length: [u8; 4] = [0, 0, 0, 0];
        match reader.read_exact(&mut length).await {
            Ok(_) => (),
            Err(e) => return Err(SLSKExitCode::IoError(e)),
        }
        let length = u32::from_le_bytes(length);
        let mut bytes: Vec<u8> = vec![0; length as usize];

        match reader.read_exact(&mut bytes).await {
            Ok(_) => (),
            Err(e) => return Err(SLSKExitCode::IoError(e)),
        }
        Ok(match message_type {
            MessageType::Server(_) => (
                MessageType::Server(match <u32>::unpack_from_bytes(&mut bytes) {
                    Some(n) => n,
                    None => return Err(SLSKExitCode::IoError(Error::from(ErrorKind::InvalidData))),
                }),
                bytes,
            ),
            MessageType::PeerInit(_) => (
                MessageType::PeerInit(match <u8>::unpack_from_bytes(&mut bytes) {
                    Some(n) => n,
                    None => return Err(SLSKExitCode::IoError(Error::from(ErrorKind::InvalidData))),
                }),
                bytes,
            ),
            MessageType::Peer(_) => (
                MessageType::Peer(match <u32>::unpack_from_bytes(&mut bytes) {
                    Some(n) => n,
                    None => return Err(SLSKExitCode::IoError(Error::from(ErrorKind::InvalidData))),
                }),
                bytes,
            ),
            MessageType::File => unimplemented!(),
            MessageType::Distributed(_) => (
                MessageType::Distributed(match <u8>::unpack_from_bytes(&mut bytes) {
                    Some(n) => n,
                    None => return Err(SLSKExitCode::IoError(Error::from(ErrorKind::InvalidData))),
                }),
                bytes,
            ),
        })
    }

    // Spawn separate tasks for reading and writing
    let server_read_task = tokio::spawn(async move {
        loop {
            if *quit.read().await {
                return SLSKExitCode::Ok;
            };
            let (code, mut bytes) =
                match get_code_and_bytes_from_readable(&mut reader, MessageType::Server(0)).await {
                    Ok((code, bytes)) => (code, bytes),
                    Err(e) => return e,
                };

            match code {
                MessageType::Server(1) => {
                    if let Some(response) = Login::from_stream(&mut bytes) {
                        let _ = write_queue.send(SLSKEvents::LoginResult {
                            success: response.success,
                            reason: response.failure_reason,
                        });
                        if !response.success {
                            drop(reader);
                            return SLSKExitCode::LoginFail;
                        }
                        *logged_in.lock().await = true;
                    }
                }
                MessageType::Server(3) => {
                    if let Some(response) = GetPeerAddress::from_stream(&mut bytes) {
                        user_info_map
                            .lock()
                            .await
                            .insert(response.username.clone(), (response.ip, response.port));
                    }
                    // println!("{:#?}", GetPeerAddress::from_stream(&mut bytes));
                }
                MessageType::Server(5) => {
                    // println!("{:#?}", WatchUser::from_stream(&mut bytes));
                }
                MessageType::Server(7) => {
                    // println!("{:#?}", GetUserStatus::from_stream(&mut bytes));
                }
                MessageType::Server(13) => {
                    if let Some(response) = SayChatroom::from_stream(&mut bytes) {
                        let _ = write_queue.send(SLSKEvents::ChatroomMessage {
                            room: response.room,
                            username: Some(response.username),
                            message: response.message,
                        });
                    }
                }
                MessageType::Server(14) => {
                    if let Some(response) = JoinRoom::from_stream(&mut bytes) {
                        let _ = write_queue.send(SLSKEvents::UpdateRoom {
                            room: response.room,
                            stats: response
                                .usernames
                                .into_iter()
                                .zip(response.stats)
                                .collect::<Vec<(String, UserStats)>>(),
                        });
                    }
                }
                MessageType::Server(15) => {
                    // println!("{:#?}", LeaveRoom::from_stream(&mut bytes));
                }
                MessageType::Server(16) => {
                    // println!("{:#?}", UserJoinedRoom::from_stream(&mut bytes));
                }
                MessageType::Server(17) => {
                    // println!("{:#?}", UserLefRoom::from_stream(&mut bytes));
                }
                MessageType::Server(18) => {
                    // Ideally, this shouldn't happen if we receive the PeerInit message
                    // While still testing stuff out it's not the end of the world
                    // TODO: Handle peer connections appropriately depending on if we have an open port
                    if let Some(connect_request) = ConnectToPeer::from_stream(&mut bytes) {
                        match connect_request.connection_type {
                            constants::ConnectionTypes::PeerToPeer => {
                                let _ = indirect_peers_list_writer.push(connect_request);
                            }
                            constants::ConnectionTypes::FileTransfer => {
                                let _ = indirect_peers_list_writer.push(connect_request);
                            }
                            constants::ConnectionTypes::DistributedNetwork => (),
                        }
                    }
                }
                MessageType::Server(22) => {
                    // println!("{:#?}", MessageUser::from_stream(&mut bytes));
                }
                MessageType::Server(26) => {
                    // println!("{:#?}", FileSearch::from_stream(&mut bytes));
                }
                MessageType::Server(36) => {
                    // println!("{:#?}", GetUserStats::from_stream(&mut bytes));
                }
                MessageType::Server(41) => {
                    // println!("{:#?}", Relogged::from_stream(&mut bytes));
                    return SLSKExitCode::OtherLogin;
                }
                MessageType::Server(64) => {
                    if let Some(room_list) = RoomList::from_stream(&mut bytes) {
                        let rooms_and_num_of_users = room_list
                            .rooms
                            .into_iter()
                            .zip(room_list.num_of_users)
                            .collect();
                        let _ = write_queue.send(SLSKEvents::RoomList {
                            rooms_and_num_of_users,
                        });
                    }
                }
                MessageType::Server(66) => {
                    // println!("{:#?}", AdminMessage::from_stream(&mut bytes));
                }
                MessageType::Server(69) => {
                    // println!("{:#?}", PrivilegedUsers::from_stream(&mut bytes));
                }
                MessageType::Server(83) => {
                    // println!("{:#?}", ParentMinSpeed::from_stream(&mut bytes));
                }
                MessageType::Server(84) => {
                    // println!("{:#?}", ParentSpeedRatio::from_stream(&mut bytes));
                }
                MessageType::Server(92) => {
                    // println!("{:#?}", CheckPrivileges::from_stream(&mut bytes));
                }
                MessageType::Server(93) => {
                    // println!("{:#?}", EmbeddedMessage::from_stream(&mut bytes));
                }
                MessageType::Server(102) => {
                    // println!("{:#?}", PossibleParents::from_stream(&mut bytes));
                }
                MessageType::Server(104) => {
                    // println!("{:#?}", WishListInterval::from_stream(&mut bytes));
                }
                MessageType::Server(113) => {
                    // println!("{:#?}", RoomTickerState::from_stream(&mut bytes));
                }
                MessageType::Server(114) => {
                    // println!("{:#?}", RoomTickerAdd::from_stream(&mut bytes));
                }
                MessageType::Server(115) => {
                    // println!("{:#?}", RoomTickerRemove::from_stream(&mut bytes));
                }
                MessageType::Server(130) => {
                    // println!("{:#?}", ResetDistributed::from_stream(&mut bytes));
                }
                MessageType::Server(133) => {
                    // println!("{:#?}", PrivateRoomUsers::from_stream(&mut bytes));
                }
                MessageType::Server(134) => {
                    // println!("{:#?}", PrivateRoomAddUser::from_stream(&mut bytes));
                }
                MessageType::Server(135) => {
                    // println!("{:#?}", PrivateRoomRemoveUser::from_stream(&mut bytes));
                }
                MessageType::Server(139) => {
                    // println!("{:#?}", PrivateRoomAdded::from_stream(&mut bytes));
                }
                MessageType::Server(140) => {
                    // println!("{:#?}", PrivateRoomRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(141) => {
                    // println!("{:#?}", PrivateRoomToggle::from_stream(&mut bytes));
                }
                MessageType::Server(142) => {
                    // println!("{:#?}", ChangePassword::from_stream(&mut bytes));
                }
                MessageType::Server(143) => {
                    // println!("{:#?}", PrivateRoomAddOperator::from_stream(&mut bytes));
                }
                MessageType::Server(144) => {
                    // println!("{:#?}", PrivateRoomRemoveOperator::from_stream(&mut bytes));
                }
                MessageType::Server(145) => {
                    // println!("{:#?}", PrivateRoomOperatorAdded::from_stream(&mut bytes));
                }
                MessageType::Server(146) => {
                    // println!("{:#?}", PrivateRoomOperatorRemoved::from_stream(&mut bytes));
                }
                MessageType::Server(148) => {
                    // println!("{:#?}", PrivateRoomOwned::from_stream(&mut bytes));
                }
                MessageType::Server(1001) => {
                    println!("cant connect to peer {bytes:?} ");
                    print!("{:#?}", CantConnectToPeer::from_stream(&mut bytes));
                }
                MessageType::Server(1003) => {
                    // println!("{:#?}", CantConnectToRoom::from_stream(&mut bytes));
                }
                _ => (),
            }
        }
    });

    let server_write_task = tokio::spawn({
        let my_username = my_username.clone();
        async move {
            loop {
                sleep(Duration::from_nanos(1)).await;
                let event = read_queue.recv().await;
                match event {
                    Ok(event) => match event {
                        SLSKEvents::TryLogin { username, password } => {
                            *my_username.lock().await = Some(username.clone());
                            let login_info = _SendLogin::new(username, password);
                            let _ = block_on(Login::async_write_to(&mut writer, login_info).await);
                        }
                        SLSKEvents::Quit { restart } => {
                            *quit_write.write().await = true;
                            let _ = writer.shutdown();
                            return if restart {
                                SLSKExitCode::Ok
                            } else {
                                SLSKExitCode::Restart
                            };
                        }
                        SLSKEvents::LoginResult { success, .. } => {
                            if success {
                                let _ = block_on(
                                    SetWaitPort::async_write_to(
                                        &mut writer,
                                        SetWaitPort {
                                            port: my_port,
                                            _unknown: 0,
                                            obfuscated_port: my_port, // TODO: Implement SetWaitPort obfuscated_port
                                        },
                                    )
                                    .await,
                                );
                                let _ = block_on(
                                    RoomList::async_write_to(&mut writer, _SendRoomList {}).await,
                                );
                            }
                        }
                        SLSKEvents::RoomList { .. } => (),
                        SLSKEvents::JoinRoom { room, private } => {
                            let _ = block_on(
                                JoinRoom::async_write_to(
                                    &mut writer,
                                    _SendJoinRoom { room, private },
                                )
                                .await,
                            );
                        }
                        SLSKEvents::LeaveRoom { room } => {
                            let _ = block_on(
                                LeaveRoom::async_write_to(&mut writer, _SendLeaveRoom { room })
                                    .await,
                            );
                        }
                        SLSKEvents::UpdateRoom { .. } => (),
                        SLSKEvents::ChatroomMessage {
                            room,
                            username,
                            message,
                        } => match username {
                            Some(_) => (),
                            None => {
                                let _ = block_on(
                                    SayChatroom::async_write_to(
                                        &mut writer,
                                        _SendSayChatroom { room, message },
                                    )
                                    .await,
                                );
                            }
                        },
                        SLSKEvents::SearchResults { .. } => (),
                        SLSKEvents::FileSearch { query, token } => {
                            let _ = block_on(
                                FileSearch::async_write_to(
                                    &mut writer,
                                    _SendFileSearch {
                                        token,
                                        search_query: query,
                                    },
                                )
                                .await,
                            );
                        }
                        SLSKEvents::QueueMessage {
                            token,
                            message_bytes,
                        } => {
                            token_message_map
                                .lock()
                                .await
                                .entry(token)
                                .or_default()
                                .push_back(message_bytes);
                        }
                        SLSKEvents::Connect {
                            username,
                            token,
                            connection_type,
                        } => {
                            if !writer_user_info_map.lock().await.contains_key(&username) {
                                writer_write_queue
                                    .send(SLSKEvents::GetInfo(username.clone()))
                                    .unwrap();
                            };
                            prompted_peers_list_writer.push((username, token, connection_type));
                        }
                        SLSKEvents::GetInfo(username) => {
                            let _ = block_on(
                                GetPeerAddress::async_write_to(
                                    &mut writer,
                                    _SendGetPeerAddress { username },
                                )
                                .await,
                            );
                        }
                        SLSKEvents::NewDownloads { .. } => (),
                        SLSKEvents::NewDownload { .. } => (),
                        SLSKEvents::UpdateDownload {
                            filename,
                            status,
                            percentage,
                        } => {
                            download_filename_map
                                .lock()
                                .await
                                .entry(filename)
                                .or_default()
                                .push_back((status, percentage, None));
                        }
                        SLSKEvents::UpdateDownloads { files, from_all } => {
                            let mut download_filename_map = download_filename_map.lock().await;
                            for (filename, status, percentage) in files {
                                download_filename_map
                                    .entry(filename)
                                    .or_default()
                                    .push_back((status, percentage, Some(from_all)));
                            }
                        }
                    },
                    Err(_) => {
                        *quit_write.write().await = true;
                        let _ = writer.shutdown();
                        return SLSKExitCode::Ok;
                    }
                };
            }
        }
    });

    let listener_task = tokio::spawn(async move {
        loop {
            sleep(Duration::from_nanos(1)).await;
            match listener.accept().await {
                Ok((mut peer_stream, _peer_addr)) => {
                    // If we receive unhandled connection requests from the previous session
                    if !*logged_in_listener.lock().await {
                        let _ = peer_stream.shutdown();
                        continue;
                    }

                    match get_code_and_bytes_from_readable(
                        &mut peer_stream,
                        MessageType::PeerInit(0),
                    )
                    .await
                    {
                        Ok((code, mut bytes)) => match code {
                            MessageType::PeerInit(0) => {
                                if let Some(response) = PierceFireWall::from_stream(&mut bytes) {
                                    println!("we received a piercefirewall: {response:?}");
                                    todo!()
                                }
                            }
                            MessageType::PeerInit(1) => {
                                if let Some(response) = PeerInit::from_stream(&mut bytes) {
                                    println!(
                                        "received peerinit with type {:?} from {}",
                                        response.connection_type, response.username
                                    );
                                    let _ = direct_peers_list_writer.push((
                                        peer_stream,
                                        response.username,
                                        response.token,
                                        response.connection_type,
                                    ));
                                }
                            }
                            _ => (),
                        },
                        Err(_) => (),
                    };
                }
                Err(e) => eprintln!("{e:?}"),
            }
        }
    });

    // TODO: Handle old peers better
    // Peers who try to send data from a previous search (now deleted/invalid) still get sent to the queue.
    // This slows down receiving data that is actually desired.
    // To combat this, there should be some list of currently valid tokens,
    // if data is then sent with an invalid token, the connection should be closed/the data should be ignored.
    let my_username = my_username.clone();
    let peer_task = tokio::spawn({
        async move {
            let results_map = Arc::new(Mutex::new(HashMap::<u32, u32>::new()));
            let tcp_queue = crossbeam_deque::Worker::<(
                String,
                u32,
                tokio::net::TcpStream,
                ConnectionTypes,
            )>::new_fifo();
            let tcp_reader = tcp_queue.stealer();
            let tcp_queue = Arc::new(Mutex::new(tcp_queue));

            let _connection_task = tokio::spawn(async move {
                loop {
                    let _prompted = loop {
                        match prompted_peers_list_reader.steal() {
                            crossbeam_deque::Steal::Empty => break,
                            crossbeam_deque::Steal::Success((username, token, connection_type)) => {
                                let mut count: u16 = 0;
                                loop {
                                    sleep(Duration::from_millis(10)).await;

                                    if let Some((ip, port)) =
                                        peer_user_info_map.lock().await.get(&username).cloned()
                                    {
                                        let my_username = my_username.clone();
                                        let tcp_queue = tcp_queue.clone();
                                        tokio::spawn(async move {
                                            if let Ok(Ok(mut peer_stream)) = tokio::time::timeout(
                                                Duration::from_secs(CONNECTION_TIME),
                                                tokio::net::TcpStream::connect(format!(
                                                    "{}:{port}",
                                                    ip.to_string()
                                                )),
                                            )
                                            .await
                                            {
                                                block_on(
                                                    PeerInit::async_write_to(
                                                        &mut peer_stream,
                                                        PeerInit {
                                                            username: my_username
                                                                .lock()
                                                                .await
                                                                .clone()
                                                                .unwrap(),
                                                            connection_type,
                                                            token: 0,
                                                        },
                                                    )
                                                    .await,
                                                )
                                                .unwrap();
                                                match connection_type {
                                                    ConnectionTypes::PeerToPeer => (),
                                                    ConnectionTypes::FileTransfer => (),
                                                    ConnectionTypes::DistributedNetwork => {
                                                        todo!("implement distributed messages")
                                                    }
                                                }
                                                tcp_queue.lock().await.push((
                                                    username,
                                                    token,
                                                    peer_stream,
                                                    connection_type,
                                                ));
                                            }
                                        });
                                        break;
                                    } else {
                                        count += 1;

                                        if count == 10 {
                                            break;
                                        }
                                    }
                                }
                            }
                            crossbeam_deque::Steal::Retry => {
                                continue;
                            }
                        }
                    };

                    let _indirect = {
                        sleep(Duration::from_millis(10)).await;
                        loop {
                            match indirect_peers_list_reader.steal() {
                                crossbeam_deque::Steal::Empty => break,
                                crossbeam_deque::Steal::Success(indirect_connection) => {
                                    let tcp_queue = tcp_queue.clone();
                                    tokio::spawn(async move {
                                        let peer_addr = format!(
                                            "{}:{}",
                                            indirect_connection.ip, indirect_connection.port
                                        );
                                        if let Ok(Ok(mut peer_stream)) = {
                                            tokio::time::timeout(
                                                Duration::from_secs(CONNECTION_TIME),
                                                tokio::net::TcpStream::connect(&peer_addr),
                                            )
                                        }
                                        .await
                                        {
                                            match block_on(
                                                PierceFireWall::async_write_to(
                                                    &mut peer_stream,
                                                    PierceFireWall {
                                                        token: indirect_connection.firewall_token,
                                                    },
                                                )
                                                .await,
                                            ) {
                                                Ok(_) => tcp_queue.lock().await.push((
                                                    indirect_connection.username,
                                                    indirect_connection.firewall_token,
                                                    peer_stream,
                                                    indirect_connection.connection_type,
                                                )),
                                                Err(_) => {
                                                    let _ = peer_stream.shutdown().await;
                                                }
                                            }
                                        };
                                    });
                                }
                                crossbeam_deque::Steal::Retry => continue,
                            }
                        }
                    };

                    let _direct = loop {
                        sleep(Duration::from_millis(10)).await;
                        match direct_peers_list_reader.steal() {
                            crossbeam_deque::Steal::Empty => break,
                            crossbeam_deque::Steal::Success((
                                stream,
                                username,
                                token,
                                connection_type,
                            )) => {
                                tcp_queue.lock().await.push((
                                    username,
                                    token,
                                    stream,
                                    connection_type,
                                ));
                                break;
                            }
                            crossbeam_deque::Steal::Retry => continue,
                        };
                    };
                }
            });
            for task_num in 0..256u32 {
                tokio::task::spawn({
                    let peer_task_write_queue = peer_write_queue.clone();
                    let peer_token_message_map = Arc::clone(&peer_token_message_map);
                    let peer_download_filename_map = Arc::clone(&peer_download_filename_map);
                    let file_info_map = Arc::clone(&file_info_map);
                    let results_map = Arc::clone(&results_map);
                    let tcp_reader = tcp_reader.clone();

                    async move {
                        loop {
                            sleep(Duration::from_nanos(1)).await;
                            let temp_token_message_map = Arc::clone(&peer_token_message_map);
                            let file_info_map = Arc::clone(&file_info_map);
                            let results_map = Arc::clone(&results_map);

                            let (username, token, mut peer_stream, connection_type) = loop {
                                match tcp_reader.steal() {
                                    crossbeam_deque::Steal::Empty => {
                                        sleep(Duration::from_nanos(1)).await
                                    }
                                    crossbeam_deque::Steal::Success(result) => break result,
                                    crossbeam_deque::Steal::Retry => continue,
                                }
                            };

                            tokio::task::spawn({
                                let peer_task_write_queue = peer_task_write_queue.clone();
                                let peer_download_filename_map =
                                    Arc::clone(&peer_download_filename_map);
                                async move {
                                    if connection_type == ConnectionTypes::FileTransfer {
                                        let offset = 0;
                                        let mut percentage = 0u8;
                                        let mut downloaded = offset;
                                        let file_init_token =
                                            peer_stream.read_u32_le().await.unwrap();
                                        let (filename, filesize) = {
                                            file_info_map
                                                .lock()
                                                .await
                                                .get_mut(&file_init_token)
                                                .unwrap()
                                                .pop_front()
                                                .unwrap()
                                        };
                                        let (download_status, download_percentage, download_type) = {
                                            loop {
                                                {
                                                    let mut locked_peer_download_filename_map =
                                                        peer_download_filename_map.lock().await;
                                                    if let Some(all_download_info) =
                                                        locked_peer_download_filename_map
                                                            .get_mut(&filename)
                                                    {
                                                        if let Some(download_info) =
                                                            all_download_info.pop_front()
                                                        {
                                                            if all_download_info.is_empty() {
                                                                locked_peer_download_filename_map
                                                                    .remove(&filename);
                                                            }
                                                            break download_info;
                                                        }
                                                    };
                                                    sleep(Duration::from_millis(500)).await;
                                                }
                                            }
                                        };
                                        {
                                            *download_status.write().await =
                                                DownloadStatus::Starting;
                                        }
                                        let mut file_handle = std::fs::File::create({
                                            let (prefix, base_name) =
                                                filename.rsplit_once("\\").unwrap();
                                            let filepath = match download_type {
                                                Some(is_all) => {
                                                    let folder =
                                                        prefix.rsplit_once("\\").unwrap().1;

                                                    {
                                                        if is_all {
                                                            let folder_path =
                                                                Path::new(&username).join(folder);
                                                            if !std::fs::exists(&username).unwrap()
                                                            {
                                                                create_dir_all(&folder_path)
                                                                    .unwrap();
                                                            } else if !std::fs::exists(&folder_path)
                                                                .unwrap()
                                                            {
                                                                create_dir(&folder_path).unwrap();
                                                            };
                                                            folder_path
                                                        } else {
                                                            let folder_path = Path::new(&folder);
                                                            if !std::fs::exists(&folder_path)
                                                                .unwrap()
                                                            {
                                                                create_dir(&folder_path).unwrap();
                                                            };
                                                            folder_path.to_path_buf()
                                                        }
                                                    }
                                                    .join(base_name)
                                                }
                                                None => base_name.into(),
                                            };
                                            if filepath.exists() {
                                                let mut count = 1;
                                                let base_name = filepath.with_extension("");
                                                let base_name = base_name.to_string_lossy();
                                                let extension = filepath
                                                    .extension()
                                                    .map(|s| format!(".{}", s.to_string_lossy()))
                                                    .unwrap_or_default();

                                                loop {
                                                    let new_filepath = Path::new(&format!(
                                                        "{base_name} ({count}){extension}",
                                                    ))
                                                    .to_path_buf();
                                                    if new_filepath.exists() {
                                                        count += 1;
                                                    } else {
                                                        break new_filepath;
                                                    }
                                                }
                                            } else {
                                                filepath
                                            }
                                        })
                                        .unwrap();
                                        peer_stream.write_u64_le(offset).await.unwrap();

                                        loop {
                                            sleep(Duration::from_nanos(1)).await;
                                            let mut buf = vec![
                                                0;
                                                std::cmp::min(
                                                    (filesize - downloaded) as usize,
                                                    CHUNK_SIZE,
                                                )
                                            ];
                                            {
                                                *download_status.write().await =
                                                    DownloadStatus::Downloading;
                                            }
                                            match peer_stream.read_exact(&mut buf).await {
                                                Ok(n) => {
                                                    downloaded += n as u64;
                                                    file_handle.write_all(&buf).unwrap();
                                                    if downloaded == filesize {
                                                        log(format!(
                                                            "finished downloading {file_handle:?}"
                                                        ));
                                                        {
                                                            *download_status.write().await =
                                                                DownloadStatus::Complete;

                                                            *download_percentage.write().await =
                                                                Percentage(100);
                                                        }
                                                        break;
                                                    }
                                                    let new_percentge =
                                                        ((downloaded * 100) / filesize) as u8;
                                                    if new_percentge != percentage {
                                                        {
                                                            *download_percentage.write().await =
                                                                Percentage(new_percentge);
                                                        }
                                                        percentage = new_percentge;
                                                    }
                                                }
                                                Err(e) => {
                                                    log(format!("stopped downloading {file_handle:?} due to {e:?}"));
                                                    *download_status.write().await =
                                                        DownloadStatus::Failed;
                                                    break;
                                                }
                                            }
                                        }
                                        file_handle.flush().unwrap();
                                        let _ = peer_stream.shutdown().await;
                                        return;
                                    } else {
                                        if let Some(messages) =
                                            temp_token_message_map.lock().await.remove(&token)
                                        {
                                            for message in messages {
                                                log(format!("sent qu to {token} {username}"));
                                                peer_stream.write_all(&message).await.unwrap();
                                            }
                                        }

                                        loop {
                                            sleep(Duration::from_nanos(1)).await;
                                            let data = get_code_and_bytes_from_readable(
                                                &mut peer_stream,
                                                match connection_type {
                                                    ConnectionTypes::PeerToPeer => {
                                                        MessageType::Peer(0)
                                                    }
                                                    ConnectionTypes::FileTransfer => {
                                                        unreachable!()
                                                    }
                                                    ConnectionTypes::DistributedNetwork => {
                                                        MessageType::Distributed(0)
                                                    }
                                                },
                                            )
                                            .await;

                                            match data {
                                                Ok((code, mut bytes)) => {
                                                    match code {
                                                        MessageType::Peer(4) => {
                                                            if let Some(response) =
                                                                SharedFileListRequest::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                            // TODO: Get (and send) file list
                                                        }
                                                        MessageType::Peer(5) => {
                                                            if let Some(response) =
                                                                SharedFileListResponse::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(9) => {
                                                            let mut buf = Vec::new();
                                                            // log(format!("started decoding search results on {task_num} from {username}"));
                                                            match ZlibDecoder::new(&bytes[..])
                                                                .read_to_end(&mut buf)
                                                            {
                                                                Ok(_) => {
                                                                    bytes = buf;
                                                                    if let Some(response) =
                                                                    FileSearchResponse::from_stream(
                                                                        &mut bytes,
                                                                    )
                                                                {
                                                                    let mut results_map =
                                                                        results_map.lock().await;
                                                                    let num_files =
                                                                        response.files.len() as u32;
                                                                    let count = if results_map
                                                                        .contains_key(
                                                                            &response.token,
                                                                        ) {
                                                                        results_map.get_mut(
                                                                            &response.token,
                                                                        )
                                                                    } else {
                                                                        results_map.insert(
                                                                            response.token,
                                                                            0,
                                                                        );
                                                                        results_map.get_mut(
                                                                            &response.token,
                                                                        )
                                                                    }
                                                                    .unwrap();
                                                                    if *count < MAX_RESULTS
                                                                    {
                                                                        *count += num_files;
                                                                        peer_task_write_queue
                                                            .send(
                                                                events::SLSKEvents::SearchResults(
                                                                    response,
                                                                ),
                                                            )
                                                            .unwrap();
                                                                    }
                                                                };
                                                                }
                                                                Err(_) => {
                                                                    // break;
                                                                }
                                                            };
                                                            // log(format!("finished decoding search results on {task_num} from {username}"));
                                                            break;
                                                        }
                                                        MessageType::Peer(15) => {
                                                            if let Some(response) =
                                                                UserInfoRequest::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(16) => {
                                                            if let Some(response) =
                                                                UserInfoResponse::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(36) => {
                                                            if let Some(response) =
                                                                FolderContentsRequest::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(37) => {
                                                            if let Some(response) =
                                                                FolderContentsResponse::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(40) => {
                                                            if let Some(response) =
                                                                TransferRequest::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                if response.direction
                                                                == TransferDirections::UploadToPeer
                                                            {
                                                                log(format!("received transferequest from {} for {}", username, response.filename));
                                                                let size =
                                                                    response.filesize.unwrap();
                                                                {
                                                                    file_info_map
                                                                        .lock()
                                                                        .await
                                                                        .entry(response.token)
                                                                        .or_default()
                                                                        .push_back((response.filename, size));
                                                                }
                                                                let _ = block_on(
                                                                TransferResponse::async_write_to(
                                                                    &mut peer_stream,
                                                                    TransferResponse {
                                                                        token: response.token,
                                                                        reason: TransferResponseReason::Allowed(None),

                                                                    },
                                                                )
                                                                .await,
                                                            );
                                                                // break;
                                                            }
                                                            }
                                                        }
                                                        MessageType::Peer(41) => {
                                                            if let Some(response) =
                                                                TransferResponse::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("received {response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(43) => {
                                                            if let Some(response) =
                                                                QueueUpload::from_stream(&mut bytes)
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(44) => {
                                                            if let Some(response) =
                                                                PlaceInQueueResponse::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(46) => {
                                                            if let Some(response) =
                                                                UploadFailed::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                log(format!(
                                                                    "upload failed: {response:?}"
                                                                ));
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(50) => {
                                                            if let Some(response) =
                                                                UploadDenied::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(51) => {
                                                            if let Some(response) =
                                                                PlaceInQueueRequest::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(52) => {
                                                            if let Some(response) =
                                                                UploadQueueNotification::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                println!("{response:?}");
                                                                // }
                                                            }
                                                        }
                                                        unknown => {
                                                            eprintln!(
                                                                "received unknown: {unknown:?}"
                                                            )
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log(format!(
                                                        "got error {e:?} from username {username}"
                                                    ));
                                                    break;
                                                }
                                            }
                                            // loops once only unless explicitly continued
                                            // done this way to avoid long timeouts stalling peer activity
                                            // TODO: allow for more queued messages to be passed to existing connections
                                            // perhaps by chaning token_message_map to have a value of Vec<Vec<u8>> (multiple messages as bytes)
                                            break;
                                        }
                                        let _ = peer_stream.shutdown().await;
                                        // log(format!("shut down {username} in task {task_num}"));
                                    }
                                }
                            });
                        }
                    }
                });
            }
        }
    });

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
