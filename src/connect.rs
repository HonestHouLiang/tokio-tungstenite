//! Connection helper.
use std::net::{SocketAddr, ToSocketAddrs};
use tokio::net::TcpStream;
use tokio_socks::tcp::Socks5Stream;

use tungstenite::{
    error::{Error, UrlError},
    handshake::client::{Request, Response},
    protocol::WebSocketConfig,
};

use crate::{ stream::MaybeTlsStream, Connector, IntoClientRequest, WebSocketStream, client_async_tls_with_config};

/// Connect to a given URL.
pub async fn connect_async<R>(
    request: R,
    proxy: Option<std::net::SocketAddr>,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error>
    where
        R: IntoClientRequest + Unpin,
{
    connect_async_with_config(request, None, false,proxy).await
}

/// The same as `connect_async()` but the one can specify a websocket configuration.
/// Please refer to `connect_async()` for more details. `disable_nagle` specifies if
/// the Nagle's algorithm must be disabled, i.e. `set_nodelay(true)`. If you don't know
/// what the Nagle's algorithm is, better leave it set to `false`.
pub async fn connect_async_with_config<R>(
    request: R,
    config: Option<WebSocketConfig>,
    disable_nagle: bool,
    proxy: Option<std::net::SocketAddr>,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error>
    where
        R: IntoClientRequest + Unpin,
{
    connect(request.into_client_request()?, config, disable_nagle, None,proxy).await
}

/// The same as `connect_async()` but the one can specify a websocket configuration,
/// and a TLS connector to use. Please refer to `connect_async()` for more details.
/// `disable_nagle` specifies if the Nagle's algorithm must be disabled, i.e.
/// `set_nodelay(true)`. If you don't know what the Nagle's algorithm is, better
/// leave it to `false`.
#[cfg(any(feature = "native-tls", feature = "__rustls-tls"))]
pub async fn connect_async_tls_with_config<R>(
    request: R,
    config: Option<WebSocketConfig>,
    disable_nagle: bool,
    connector: Option<Connector>,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error>
    where
        R: IntoClientRequest + Unpin,
{
    connect(request.into_client_request()?, config, disable_nagle, connector,None).await
}

async fn connect(
    request: Request,
    config: Option<WebSocketConfig>,
    disable_nagle: bool,
    connector: Option<Connector>,
    proxy: Option<std::net::SocketAddr>,
) -> Result<(WebSocketStream<MaybeTlsStream<TcpStream>>, Response), Error> {
    let uri = request.uri();
    let domain = uri.host().ok_or(Error::Url(UrlError::NoHostName))?;
    let port = uri
        .port_u16()
        .or_else(|| match request.uri().scheme_str() {
            Some("wss") => Some(443),
            Some("ws") => Some(80),
            _ => None,
        })
        .ok_or(Error::Url(UrlError::UnsupportedUrlScheme))?;

    let stream = match proxy {
        None => {
            println!("没有代理");
            let stream = connect_to(domain, port).await?;
            stream
        }
        Some(p) => {
            println!("代理");
            let stream = connect_to_proxy(domain, port, p).await?;
            stream
        }
    };
    if disable_nagle {
        stream.set_nodelay(true)?
    }
    client_async_tls_with_config(request, stream, config, connector).await
}


//获取不走代理的 TcpStream
async fn connect_to(domain: &str, port: u16) -> Result<TcpStream, Error> {
    let addr = format!("{domain}:{port}");
    println!("域名解析：{}", addr);
    if let Ok(stream) = TcpStream::connect(addr).await {
        return Ok(stream);
    }
    Err(Error::Url(UrlError::UnableToConnect("域解析地址都不可用".to_string())))
}

//获取代理 的TcpStream
async fn connect_to_proxy(domain: &str, port: u16, proxy: SocketAddr) -> Result<TcpStream, Error> {
    let addrs = (domain, port).to_socket_addrs().unwrap();
    for addr in addrs.as_slice() {
        println!("域名解析：{}", addr);
        if let Ok(socks5_stream) = Socks5Stream::connect(proxy, (domain, port)).await {
            let stream = socks5_stream.into_inner();
            return Ok(stream);
        }
    }
    Err(Error::Url(UrlError::UnableToConnect("域解析地址都不可用".to_string())))
}
