#![cfg(feature = "testing")]
use crate::service::create_service;
use crate::storage::Storage;
use fdb_testcontainer::{DatabaseGuardOnce, get_db_once};
use prost::Message;
use pusu_protocol::response::create_auth_response_struct;
use pusu_toolbox::create_biscuit_with_keypair;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The `Server` struct represents a test server setup for the application.
///
/// It is designed to initialize and manage the test server, including the creation
/// of an in-memory database guard and a Biscuit keypair. The server facilitates communication
/// with clients for testing purposes.
///
/// ## Fields
/// - `_guard`: A `DatabaseGuardOnce` instance that ensures the database is properly initialized
///   and maintained during the lifetime of the server.
/// - `keypair`: A Biscuit keypair used for authentication purposes. The public key is shared
///   with services interacting with the server.
/// - `server`: A handle to the test server that allows connection and communication.
///
/// ## Methods
/// - `new`: Asynchronously creates a new server instance, initializing the database, Biscuit
///   keypair, and starting the test server.
/// - `get_biscuit`: Generates a Biscuit token for a specified tenant, using the server's keypair.
/// - `get_connection`: Establishes a connection to the server and returns the connected `TcpStream`.
/// - `get_client`: Asynchronously retrieves a client instance that can be used for requests,
///   validating the server’s initial response.
pub struct Server {
    _guard: DatabaseGuardOnce,
    keypair: biscuit_auth::KeyPair,
    server: pusu_toolbox::test_server::TestServerHandle,
}

impl Server {
    /// Asynchronously creates a new instance of the `Server` struct.
    ///
    /// This constructor initializes the following components:
    /// - A guarded in-memory FoundationDB instance using `fdb-testcontainer` to simulate
    ///   database interactions for testing purposes.
    /// - A Biscuit keypair for secure authentication, used to create and verify tokens.
    /// - A test server that simulates the application server, configured with the initialized
    ///   storage backend and public key for authentication.
    ///
    /// ## Returns
    /// A `Server` instance that is fully initialized and ready for use in tests.
    ///
    /// ## Panics
    /// - If initializing the test database or test server fails, this function will panic.
    pub async fn new() -> Self {
        let _guard = get_db_once().await;
        let keypair = biscuit_auth::KeyPair::new();
        let storage = Storage::new(_guard.clone());
        let public_key = keypair.public();
        let server = pusu_toolbox::test_server::TestServer::start(move || {
            create_service(storage.clone(), public_key)
        });
        Self {
            _guard,
            keypair,
            server,
        }
    }

    /// Generates a Biscuit token for a specified tenant.
    ///
    /// This method uses the server's keypair to create a Biscuit token, which
    /// can be used for authentication and authorization purposes in testing.
    ///
    /// ## Parameters
    /// - `tenant`: A string slice representing the tenant for which the Biscuit
    ///   token will be created.
    ///
    /// ## Returns
    /// A `Biscuit` token associated with the given tenant.
    ///
    /// ## Panics
    /// This method will panic if the Biscuit token cannot be created.
    pub fn get_biscuit(&self, tenant: &str) -> biscuit_auth::Biscuit {
        create_biscuit_with_keypair(tenant, &self.keypair).expect("Unable to create the Biscuit")
    }

    /// Establishes a connection to the test server and returns the connected `TcpStream`.
    ///
    /// This method is used to create a connection to the server that allows interaction
    /// with the test server for testing purposes. The connection can be used to send
    /// requests and receive responses during testing.
    ///
    /// ## Returns
    /// A `TcpStream` representing the connection to the server.
    ///
    /// ## Panics
    /// This method will panic if unable to connect to the server.
    fn get_connection(&self) -> TcpStream {
        self.server
            .connect()
            .expect("Unable to connect to the server")
    }

    /// Asynchronously retrieves a client instance for communication with the test server.
    ///
    /// This method facilitates the testing process by connecting to the test server,
    /// reading the initial response, and validating it. A client instance is then
    /// returned, allowing for interaction with the test server by sending requests
    /// and receiving responses.
    ///
    /// ## Returns
    /// A `Client` instance that can be used to interact with the test server.
    ///
    /// ## Panics
    /// This method will panic if the connection to the server fails,
    /// the initial response cannot be read or decoded, or the response is invalid.
    pub async fn get_client(&self) -> Client {
        let mut connection = self.get_connection();
        let mut buf = Vec::new();
        connection.read_buf(&mut buf).await.expect("read");
        let response = pusu_protocol::pusu::Response::decode(&*buf).expect("decode");
        assert_eq!(response, create_auth_response_struct());
        Client { connection }
    }
}

/// The `Client` struct is used to represent a client connection to the test server.
///
/// This struct facilitates communication with the test server by sending requests
/// and receiving responses. It provides a convenient way to interact with the server
/// during testing.
///
/// ## Fields
/// - `connection`: A `TcpStream` instance representing the underlying connection to the server.
///
/// ## Methods
/// - `call`: Asynchronously sends a request to the server and retrieves a response.
///
/// ## Usage
/// The client is designed to simulate interactions with the test server, allowing
/// developers to test server functionality in isolation.
pub struct Client {
    connection: TcpStream,
}

impl Client {
    pub fn into_inner(self) -> TcpStream {
        self.connection
    }
}

impl Client {
    /// Sends a request to the test server and retrieves a response.
    ///
    /// This method facilitates interaction with the test server by asynchronously
    /// sending a serialized request and reading the server's response. The response
    /// is then decoded and returned as a `pusu_protocol::pusu::Response`.
    ///
    /// ## Parameters
    /// - `request`: A vector of bytes (`Vec<u8>`) that represents the serialized
    ///   request to be sent to the server.
    ///
    /// ## Returns
    /// A `pusu_protocol::pusu::Response` representing the server's response to the
    /// provided request.
    ///
    /// ## Panics
    /// This method will panic if:
    /// - The request cannot be sent to the server.
    /// - The response cannot be read from the server.
    /// - The response cannot be properly decoded.
    pub async fn call(&mut self, request: Vec<u8>) -> pusu_protocol::pusu::Response {
        self.connection
            .write_all(&request)
            .await
            .expect("Unable to send request");

        let mut response = vec![0; 10024];
        let response_size = self
            .connection
            .read(&mut response)
            .await
            .expect("Unable to read response");
        pusu_protocol::pusu::Response::decode(&response[..response_size])
            .expect("Unable to decode response")
    }
}
