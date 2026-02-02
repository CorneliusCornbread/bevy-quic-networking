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
