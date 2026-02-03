# bevy-quic-networking
A plugin for Bevy which implements components for the use of sending data over the QUIC protocol using Amazon's [s2n-quic](https://github.com/aws/s2n-quic).

# Getting Started
First start by selecting the relevant version of the crate based on the version of Aeronet and Bevy you are using. If you are not using Aeronet you can ignore the Aeronet version.

| Bevy Version | Aeronet Version (optional) | Crate Version |
|--------------|-----------------|---------------|
| 0.18         | 0.19            | 0.18          |

You can add the crate to your project with the following addition to your `cargo.toml`
```toml
[dependencies]
bevy-quic-networking = "0.19"
```

And by adding the default plugins
```rs
let app = App::new()
    .add_plugins(QuicDefaultPlugins)
```

And for **Aeronet** functionality you will need the Aeronet plugin
```rs
let app = App::new()
    .add_plugins(QuicAeronetPlugins)
```

You can open a server in the following way:
```rs
let server_comp = QuicServer::bind(&runtime, ip, cert_path, key_path)
        .expect("Unable to bind to server address");

    commands.spawn(server_comp);
```

We can then spawn a client component:
```rs
// Spawn a new client with our cert
let mut client_comp = QuicClient::new_with_tls(&runtime, cert_path).expect("Invalid cert");
let connect = Connect::new(ip).with_server_name("localhost");
let conn_bundle = client_comp.open_connection(connect);

// Spawn client with connection attempt as child
commands.spawn(client_comp).with_children(|parent| {
    parent.spawn(conn_bundle);
});
```

You'll have to query for completed connections. Automatically the default plugins will convert successful attempts into full connection components:
```rs
fn client_open_stream(
    mut commands: Commands,
    connection_query: Query<(Entity, &mut QuicClientConnection), Without<Children>>,
) {
    // As soon as we see a connection without any children,
    // attempt to spawn a bidirectional stream on the client end.
    // Both servers and clients may open streams.
    for (entity, mut connection) in connection_query {
        let stream_bundle = connection.open_bidrectional_stream();
        commands.spawn((stream_bundle, DebugCount(0), ChildOf(entity)));
    }
}
```
