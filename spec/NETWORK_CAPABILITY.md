# Experimental network capability

```text
struct Request{method:String,path:String,query:String,headers:Map<String,String>,body:String}
struct Response{status:I,headers:Map<String,String>,body:String}
serve(addr:String,limit:I,handler:(Request)->Response) -> Result<Unit,IoError>
http_request(method:String,url:String,headers:Map<String,String>,body:String) -> Result<Response,IoError>
```

Raw TCP uses an opaque `Conn` handle:

```text
tcp_connect(addr:String) -> Result<Conn,IoError>
tcp_send(c:Conn,data:Bytes) -> Result<Unit,IoError>
tcp_recv(c:Conn,max:I) -> Result<Bytes,IoError>
tcp_close(c:Conn) -> Unit
```

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

`http_request` sends one request to an `http://host[:port]/path?query` URL
(port 80 by default) with `host`, `connection: close`, and `content-length`
set by the runtime, then reads the whole response. It decodes
`transfer-encoding: chunked` and honors `content-length`. HTTPS, redirects,
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
