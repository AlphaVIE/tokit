# Experimental network capability

```text
struct Request{method:String,path:String,query:String,headers:Map<String,String>,body:String}
struct Response{status:I,headers:Map<String,String>,body:String}
serve(addr:String,limit:I,handler:(Request)->Response) -> Result<Unit,IoError>
serve(addr:String,limit:I,workers:I,handler:(Request)->Response) -> Result<Unit,IoError>
http_request(method:String,url:String,headers:Map<String,String>,body:String) -> Result<Response,IoError>
```

Raw TCP uses an opaque `Conn` handle:

```text
tcp_connect(addr:String) -> Result<Conn,IoError>
tcp_send(c:Conn,data:Bytes) -> Result<Unit,IoError>
tcp_recv(c:Conn,max:I) -> Result<Bytes,IoError>
tcp_close(c:Conn) -> Unit
```

Servers that keep state between requests, or speak other protocols, accept
connections themselves:

```text
listen(addr:String) -> Result<Listener,IoError>
accept(l:Listener) -> Result<Conn,IoError>
http_read(c:Conn) -> Result<Request,IoError>
http_write(c:Conn,r:Response) -> Result<Unit,IoError>
```

`listen` binds `addr` (port `0` picks a free port) and needs the same grant as
`serve`; `accept` waits for the next connection and returns an ordinary
`Conn`. `http_read` reads one request with the limits and `400` rules of
`serve` (a malformed request yields `IoError::Other`, a non-UTF-8 body
`IoError::InvalidUtf8`) and waits at most 30 seconds; `http_write` writes one
response formatted exactly like `serve` does, including `connection: close`.
Each connection carries one request; close it with `tcp_close`. Because the
loop is ordinary code, `var` bindings persist across requests (see
[crud_service.tok](../examples/crud_service.tok)). After reading an upgrade
request, raw `tcp_send`/`tcp_recv` on the same `Conn` can implement protocols
such as WebSocket. `Listener` is a reserved opaque type; copies share one
socket. All four functions are effects (`net.listen`).

Copies of a `Conn` share one socket, like task handles; there is no `==` on
connections. `tcp_recv` returns between one and `max` bytes, or empty `Bytes`
when the peer closed the connection; `max` must be positive. After
`tcp_close`, sends and receives return `IoError::Other`. `tcp_connect` needs
the same `--allow-net` grant as HTTP. Protocol clients such as Redis and
PostgreSQL are written in Tokit on top of these functions.

`Request` and `Response` are built-in records; their names are reserved.
Header names are lowercased and repeated headers are joined with `, `, so a
`Map` holds them without loss of meaning. Bodies are UTF-8 text; a request
or response body that is not valid UTF-8 yields `IoError::InvalidUtf8`.

`serve` binds `addr` (for example `127.0.0.1:8080`) and answers requests one
at a time with the handler's response. It returns `Ok(())` after `limit`
requests, or never when `limit<=0`. Every response is HTTP/1.1 with
`content-length` and `connection: close`; the handler's own
`content-length`, `connection`, and header values containing line breaks
are dropped. A malformed request, a `transfer-encoding` request body, a head
over 64 KiB, or a body over 16 MiB receives `400` without reaching the
handler. Unknown status codes keep their number with a generic reason
phrase; codes outside 100–999 become `500`. A runtime failure or `exit` in
the handler ends the program as usual. Native programs flush standard output
before serving and after each request.

With a `workers` argument, native programs hand each accepted connection to
one of `workers` threads (clamped to 1–256), so slow handlers run in
parallel; `limit` then counts accepted connections, and `serve` returns after
the last of them is answered. Handlers already receive copies of captured
values, so they share no mutable state; their output lines may interleave in
any order. The reference interpreter accepts the same call and answers one
request at a time, which is one valid schedule. See
[concurrent_http.tok](../examples/concurrent_http.tok).

`http_request` also accepts `https://host[:port]/path?query` URLs (port 443
by default). HTTPS requests run through the system `curl` executable, or the
one named by `TOKIT_CURL`, which brings the platform's TLS implementation and
certificate store (Schannel on Windows, Secure Transport or OpenSSL on macOS,
OpenSSL or similar on Linux; `curl` ships with Windows 10 and later and with
macOS). Tokit checks the `--allow-net host:port` grant before starting it,
passes arguments without a shell, restricts curl to HTTPS (`--proto =https`),
applies the same 30-second and 16 MiB limits, and does not follow redirects.
A missing `curl`, a TLS or certificate failure, or a timeout yields
`IoError::Other`. WebAssembly modules cannot start processes, so HTTPS is
unavailable there.

Servers and the Redis, PostgreSQL, and WebSocket packages speak plaintext.
Expose services through a TLS-terminating reverse proxy, for example Caddy,
which obtains certificates automatically:

```text
api.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

`http_request` sends one request to an `http://host[:port]/path?query` URL
(port 80 by default) with `host`, `connection: close`, and `content-length`
set by the runtime, then reads the whole response. It decodes
`transfer-encoding: chunked` and honors `content-length`. Redirects,
proxies, and keep-alive are not supported; other URLs and protocol errors
yield `IoError::Other`.

Both functions need the `--allow-net <host:port>` grant on `tok run` or a
binary from `tok build`; the grant must equal the address being bound or
connected to, and `--allow-net '*'` allows any address. Without a matching
grant they return `Err(IoError::Denied)` before touching the network. Both
are effects (`net.listen`, `net.connect`) that spawned pure functions cannot
call (`E117`).

The reference interpreter includes the same HTTP source text that native
programs embed (`compiler/src/native_runtime/http.rs.txt`), so both backends
parse and format messages identically. See
[http_server.tok](../examples/http_server.tok). Like the filesystem grants,
this is a language-level gate, not an operating-system sandbox, and one
grant per run is a provisional interface.
