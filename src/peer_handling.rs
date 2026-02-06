use std::{
    collections::{HashMap, VecDeque},
    net::Ipv4Addr,
    sync::Arc,
    time::Duration,
};

use crossbeam_deque::{Stealer, Worker};
use smol::block_on;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    sync::{broadcast::Sender, Mutex, RwLock},
    task::JoinHandle,
    time::sleep,
};

use crate::{
    constants::{ConnectionTypes, DownloadStatus, Percentage, MAX_RESULTS},
    file_transfer::handle_file_transfer,
    events::SLSKEvents,
    messages::{
        FileSearchResponse, FolderContentsRequest, FolderContentsResponse, MessageTrait,
        MessageType, PeerInit, PierceFireWall, SharedFileListResponse, TransferRequest,
        TransferResponse, TransferResponseReason, UserInfoRequest, UserInfoResponse,
        _ReceiveConnectToPeer,
    },
    utils::{get_code_and_bytes_from_readable, log},
    PlaceInQueueRequest, PlaceInQueueResponse, QueueUpload, SLSKExitCode, SharedFileListRequest,
    TransferDirections, UploadDenied, UploadFailed, UploadQueueNotification,
    CONNECTION_TIME,
};

/// Listens for connection attempts from peers and writes them to the queue
pub(crate) async fn start_listener_task(
    listener: TcpListener,
    logged_in_listener: Arc<RwLock<bool>>,
    direct_peers_list_writer: Worker<(TcpStream, String, u32, ConnectionTypes)>,
) -> JoinHandle<SLSKExitCode> {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_nanos(1)).await;
            match listener.accept().await {
                Ok((mut peer_stream, _peer_addr)) => {
                    // If we receive unhandled connection requests from the previous session
                    if !*logged_in_listener.read().await {
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
    })
}

/// Gets pending peer connections from the queue, connects to peers and sends/receives messages
pub(crate) async fn start_peer_task(
    prompted_peers_list_reader: Stealer<(String, u32, ConnectionTypes)>,
    indirect_peers_list_reader: Stealer<_ReceiveConnectToPeer>,
    direct_peers_list_reader: Stealer<(TcpStream, String, u32, ConnectionTypes)>,
    peer_user_info_map: Arc<Mutex<HashMap<String, (Ipv4Addr, u32)>>>,
    my_username: Arc<RwLock<Option<String>>>,
    peer_write_queue: Sender<SLSKEvents>,
    peer_token_message_map: Arc<Mutex<HashMap<u32, VecDeque<Vec<u8>>>>>,
    file_info_map: Arc<Mutex<HashMap<u32, VecDeque<(String, u64)>>>>,
    peer_download_filename_map: Arc<
        Mutex<
            HashMap<
                String,
                VecDeque<(
                    Arc<RwLock<DownloadStatus>>,
                    Arc<RwLock<Percentage>>,
                    Option<bool>,
                )>,
            >,
        >,
    >,
    shares_message: Arc<RwLock<Option<Vec<u8>>>>,
) -> JoinHandle<()> {
    tokio::spawn({
        async move {
            let results_map = Arc::new(Mutex::new(HashMap::<u32, u32>::new()));
            let tcp_queue =
                Worker::<(String, u32, tokio::net::TcpStream, ConnectionTypes)>::new_fifo();
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
                                                                .read()
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

                                        if count == 100 {
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
            for _task_num in 0..256u32 {
                tokio::task::spawn({
                    let peer_task_write_queue = peer_write_queue.clone();
                    let peer_token_message_map = Arc::clone(&peer_token_message_map);
                    let peer_download_filename_map = Arc::clone(&peer_download_filename_map);
                    let file_info_map = Arc::clone(&file_info_map);
                    let results_map = Arc::clone(&results_map);
                    let tcp_reader = tcp_reader.clone();
                    let shares_message = Arc::clone(&shares_message);

                    async move {
                        loop {
                            sleep(Duration::from_nanos(1)).await;
                            let temp_token_message_map = Arc::clone(&peer_token_message_map);
                            let file_info_map = Arc::clone(&file_info_map);
                            let results_map = Arc::clone(&results_map);

                            // TODO: This works with downloads, but uploads need to be implemented too
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
                                let shares_message = Arc::clone(&shares_message);
                                async move {
                                    if connection_type == ConnectionTypes::FileTransfer {
                                        handle_file_transfer(
                                            peer_stream,
                                            file_info_map,
                                            peer_download_filename_map,
                                            username,
                                        )
                                        .await;
                                    } else {
                                        // handle regular peer messages
                                        if let Some(messages) =
                                            temp_token_message_map.lock().await.remove(&token)
                                        {
                                            for message in messages {
                                                log(format!(
                                                    "sent to {token} {username}: {message:?}"
                                                ));
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
                                                            if SharedFileListRequest::from_stream(
                                                                &mut bytes,
                                                            )
                                                            .is_some()
                                                            {
                                                                if let Some(shares_message) =
                                                                    shares_message
                                                                        .read()
                                                                        .await
                                                                        .to_owned()
                                                                {
                                                                    let _ = block_on(
                                                                        peer_stream.write_all(
                                                                            &shares_message,
                                                                        ),
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        MessageType::Peer(5) => {
                                                            if let Some(_response) =
                                                                SharedFileListResponse::from_stream(
                                                                    &mut bytes,
                                                                )
                                                            {
                                                                // loop {
                                                                // println!("{response:?}");
                                                                // TODO: UI events for SharedFileListResponse
                                                                // }
                                                            }
                                                        }
                                                        MessageType::Peer(9) => {
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
                                                                    .contains_key(&response.token)
                                                                {
                                                                    results_map
                                                                        .get_mut(&response.token)
                                                                } else {
                                                                    results_map
                                                                        .insert(response.token, 0);
                                                                    results_map
                                                                        .get_mut(&response.token)
                                                                }
                                                                .unwrap();
                                                                if *count < MAX_RESULTS {
                                                                    *count += num_files;
                                                                    peer_task_write_queue
                                                            .send(
                                                                SLSKEvents::SearchResults(
                                                                    response,
                                                                ),
                                                            )
                                                            .unwrap();
                                                                }
                                                            };
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
                                            // perhaps by changing token_message_map to have a value of Vec<Vec<u8>> (multiple messages as bytes)
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
    })
}
